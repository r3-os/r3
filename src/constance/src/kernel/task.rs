//! Tasks
use core::{cell::UnsafeCell, fmt, hash, marker::PhantomData, mem, sync::atomic::Ordering};
use num_traits::ToPrimitive;

use super::{
    hunk::Hunk, state, utils, wait, ActivateTaskError, BadIdError, ExitTaskError,
    GetCurrentTaskError, Id, InterruptTaskError, Kernel, KernelCfg1, ParkError, Port,
    PortThreading, UnparkError, UnparkExactError, WaitTimeoutError,
};
use crate::utils::{
    intrusive_list::{CellLike, Ident, ListAccessorCell, Static, StaticLink, StaticListHead},
    Init, PrioBitmap,
};

#[cfg_attr(doc, svgbobdoc::transform)]
/// Represents a single task in a system.
///
/// This type is ABI-compatible with [`Id`].
///
/// <div class="admonition-follows"></div>
///
/// > **Relation to Other Specifications:** Present in almost every real-time
/// > operating system.
///
/// # Task States
///
/// A task may be in one of the following states:
///
///  - **Dormant** — The task is not executing, doesn't have an associated
///    execution [thread], and can be [activated].
///
///  - **Ready** — The task has an associated execution thread, which is ready to
///    be scheduled to the CPU
///
///  - **Running** — The task has an associated execution thread, which is
///    currently scheduled to the CPU
///
///  - **Waiting** — The task has an associated execution thread, which is
///    currently blocked by a blocking operation
///
/// <center>
/// ```svgbob
///                     ,-------,
///    ,--------------->| Ready |<--------------,
///    |                '-------'               |
///    |          dispatch | ^                  |
///    |                   | |                  |
///    | release           | |                  | activate
/// ,---------,            | |           ,---------,
/// | Waiting |            | |           | Dormant |
/// '---------'            | |           '---------'
///    ^                   | |                  ^
///    |                   | |                  |
///    |                   v | preempt          |
///    |          wait ,---------,              |
///    '---------------| Running |--------------'
///                    '---------' exit
/// ```
/// </center>
///
/// [thread]: crate#threads
/// [activated]: Task::activate
#[repr(transparent)]
pub struct Task<System>(Id, PhantomData<System>);

// FIXME: `svgbobdoc` doesn't like `#[doc(include = ...)]`
#[doc(include = "../common.md")]
impl<System> Clone for Task<System> {
    fn clone(&self) -> Self {
        Self(self.0, self.1)
    }
}

impl<System> Copy for Task<System> {}

impl<System> PartialEq for Task<System> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<System> Eq for Task<System> {}

impl<System> hash::Hash for Task<System> {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        hash::Hash::hash(&self.0, state);
    }
}

impl<System> fmt::Debug for Task<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Task").field(&self.0).finish()
    }
}

impl<System> Task<System> {
    /// Construct a `Task` from `Id`.
    ///
    /// # Safety
    ///
    /// The kernel can handle invalid IDs without a problem. However, the
    /// constructed `Task` may point to an object that is not intended to be
    /// manipulated except by its creator. This is usually prevented by making
    /// `Task` an opaque handle, but this safeguard can be circumvented by
    /// this method.
    ///
    /// Constructing a `Task` for a current task is allowed. This can be safely
    /// done by [`Task::current`].
    pub const unsafe fn from_id(id: Id) -> Self {
        Self(id, PhantomData)
    }

    /// Get the raw `Id` value representing this task.
    pub const fn id(self) -> Id {
        self.0
    }
}

impl<System: Kernel> Task<System> {
    /// Get the current task.
    ///
    /// In a task context, this method returns the currently running task. In an
    /// interrupt context, this method returns the interrupted task (if any).
    pub fn current() -> Result<Option<Self>, GetCurrentTaskError> {
        let _lock = utils::lock_cpu::<System>()?;
        let task_cb = if let Some(cb) = System::state().running_task() {
            cb
        } else {
            return Ok(None);
        };

        // Calculate an `Id` from the task CB pointer
        let offset_bytes =
            task_cb as *const TaskCb<_> as usize - System::task_cb_pool().as_ptr() as usize;
        let offset = offset_bytes / mem::size_of::<TaskCb<System>>();

        // Safety: Constructing a `Task` for a current task is allowed
        let task = unsafe { Self::from_id(Id::new(offset as usize + 1).unwrap()) };

        Ok(Some(task))
    }

    fn task_cb(self) -> Result<&'static TaskCb<System>, BadIdError> {
        System::get_task_cb(self.0.get() - 1).ok_or(BadIdError::BadId)
    }

    /// Start the execution of the task.
    pub fn activate(self) -> Result<(), ActivateTaskError> {
        let lock = utils::lock_cpu::<System>()?;
        let task_cb = self.task_cb()?;
        activate(lock, task_cb)
    }

    /// Interrupt any ongoing wait operations undertaken by the task.
    ///
    /// This method interrupt any ongoing system call that is blocking the task.
    /// The interrupted system call will return [`WaitError::Interrupted`] or
    /// [`WaitTimeoutError::Interrupted`].
    ///
    /// [`WaitError::Interrupted`]: crate::kernel::WaitError::Interrupted
    /// [`WaitTimeoutError::Interrupted`]: crate::kernel::WaitTimeoutError::Interrupted
    pub fn interrupt(self) -> Result<(), InterruptTaskError> {
        let mut lock = utils::lock_cpu::<System>()?;
        let task_cb = self.task_cb()?;
        wait::interrupt_task(
            lock.borrow_mut(),
            task_cb,
            Err(WaitTimeoutError::Interrupted),
        )?;

        // The task is now awake, check dispatch
        unlock_cpu_and_check_preemption(lock);

        Ok(())
    }

    /// Make the task's token available, unblocking [`Kernel::park`] now or in
    /// the future.
    ///
    /// If the token is already available, this method will return without doing
    /// anything. Use [`Task::unpark_exact`] if you need to detect this
    /// condition.
    ///
    /// If the task is currently being blocked by `Kernel::park`, the token will
    /// be immediately consumed. Otherwise, it will be consumed on a next call
    /// to `Kernel::park`.
    pub fn unpark(self) -> Result<(), UnparkError> {
        match self.unpark_exact() {
            Ok(()) | Err(UnparkExactError::QueueOverflow) => Ok(()),
            Err(UnparkExactError::BadContext) => Err(UnparkError::BadContext),
            Err(UnparkExactError::BadId) => Err(UnparkError::BadId),
            Err(UnparkExactError::BadObjectState) => Err(UnparkError::BadObjectState),
        }
    }

    /// Make *exactly* one new token available for the task, unblocking
    /// [`Kernel::park`] now or in the future.
    ///
    /// If the token is already available, this method will return
    /// [`UnparkExactError::QueueOverflow`]. Thus, this method will succeed
    /// only if it made *exactly* one token available.
    ///
    /// If the task is currently being blocked by `Kernel::park`, the token will
    /// be immediately consumed. Otherwise, it will be consumed on a next call
    /// to `Kernel::park`.
    pub fn unpark_exact(self) -> Result<(), UnparkExactError> {
        let lock = utils::lock_cpu::<System>()?;
        let task_cb = self.task_cb()?;
        unpark_exact(lock, task_cb)
    }
}

/// [`Hunk`] for a task stack.
#[repr(transparent)]
pub struct StackHunk<System>(Hunk<System, [UnsafeCell<u8>]>);

// Safety: Safe code can't access the contents. Also, the port is responsible
// for making sure `StackHunk` is used in the correct way.
unsafe impl<System> Sync for StackHunk<System> {}

impl<System: Kernel> fmt::Debug for StackHunk<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("StackHunk").field(&self.0.as_ptr()).finish()
    }
}

// TODO: Preferably `StackHunk` shouldn't be `Clone` as it strengthens the
//       safety obligation of `StackHunk::from_hunk`.
impl<System> Clone for StackHunk<System> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
impl<System> Copy for StackHunk<System> {}

// TODO: Should we allow zero-sized `StackHunk`?
impl<System> Init for StackHunk<System> {
    const INIT: Self = Self(Init::INIT);
}

impl<System> StackHunk<System> {
    /// Construct a `StackHunk` from `Hunk`.
    ///
    /// # Safety
    ///
    /// The caller is responsible for making sure the region represented by
    /// `hunk` is solely used for a single task's stack.
    ///
    /// Also, `hunk` must be properly aligned for a stack region.
    pub const unsafe fn from_hunk(hunk: Hunk<System, [UnsafeCell<u8>]>) -> Self {
        Self(hunk)
    }

    /// Get the inner `Hunk`, consuming `self`.
    pub fn into_inner(self) -> Hunk<System, [UnsafeCell<u8>]> {
        self.0
    }
}

impl<System: Kernel> StackHunk<System> {
    /// Get a raw pointer to the hunk's contents.
    pub fn as_ptr(&self) -> *mut [u8] {
        &*self.0 as *const _ as _
    }
}

/// *Task control block* - the state data of a task.
#[repr(C)]
pub struct TaskCb<
    System: PortThreading,
    PortTaskState: 'static = <System as PortThreading>::PortTaskState,
    TaskPriority: 'static = <System as KernelCfg1>::TaskPriority,
> {
    /// Get a reference to `PortTaskState` in the task control block.
    ///
    /// This is guaranteed to be placed at the beginning of the struct so that
    /// assembler code can refer to this easily.
    pub port_task_state: PortTaskState,

    /// The static properties of the task.
    pub attr: &'static TaskAttr<System>,

    pub priority: TaskPriority,

    pub(super) st: utils::CpuLockCell<System, TaskSt>,

    /// Allows `TaskCb` to participate in one of linked lists.
    ///
    ///  - In a `Ready` state, this forms the linked list headed by
    ///    [`State::task_ready_queue`].
    ///
    /// [`State::task_ready_queue`]: crate::kernel::State::task_ready_queue
    pub(super) link: utils::CpuLockCell<System, Option<StaticLink<Self>>>,

    /// The wait state of the task.
    pub(super) wait: wait::TaskWait<System>,

    /// A flag indicating whether the task has a park token or not.
    pub(super) park_token: utils::CpuLockCell<System, bool>,
}

impl<System: Port, PortTaskState: Init + 'static, TaskPriority: Init + 'static> Init
    for TaskCb<System, PortTaskState, TaskPriority>
{
    const INIT: Self = Self {
        port_task_state: Init::INIT,
        attr: &TaskAttr::INIT,
        priority: Init::INIT,
        st: Init::INIT,
        link: Init::INIT,
        wait: Init::INIT,
        park_token: Init::INIT,
    };
}

impl<System: Kernel, PortTaskState: fmt::Debug + 'static, TaskPriority: fmt::Debug + 'static>
    fmt::Debug for TaskCb<System, PortTaskState, TaskPriority>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TaskCb")
            .field("port_task_state", &self.port_task_state)
            .field("attr", self.attr)
            .field("priority", &self.priority)
            .finish()
    }
}

/// The static properties of a task.
pub struct TaskAttr<System> {
    /// The entry point of the task.
    ///
    /// # Safety
    ///
    /// This is only meant to be used by a kernel port, as a task entry point,
    /// not by user code. Using this in other ways may cause an undefined
    /// behavior.
    pub entry_point: unsafe fn(usize),

    /// The parameter supplied for `entry_point`.
    pub entry_param: usize,

    // FIXME: Ideally, `stack` should directly point to the stack region. But
    //        this is blocked by <https://github.com/rust-lang/const-eval/issues/11>
    /// The hunk representing the stack region for the task.
    pub stack: StackHunk<System>,
}

impl<System> Init for TaskAttr<System> {
    const INIT: Self = Self {
        entry_point: |_| {},
        entry_param: 0,
        stack: StackHunk::INIT,
    };
}

impl<System: Kernel> fmt::Debug for TaskAttr<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TaskAttr")
            .field("entry_point", &self.entry_point)
            .field("entry_param", &self.entry_param)
            .field("stack", &self.stack)
            .finish()
    }
}

/// Task state machine
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskSt {
    /// The task is in the Dormant state.
    Dormant,

    Ready,

    /// The task is in the Running state.
    Running,

    /// The task is in the Waiting state.
    Waiting,

    /// The task should be activated at startup. This will transition into
    /// `Ready` or `Running` before the first task is scheduled.
    PendingActivation,
}

impl Init for TaskSt {
    const INIT: Self = Self::Dormant;
}

/// Implements [`Kernel::exit_task`].
pub(super) unsafe fn exit_current_task<System: Kernel>() -> Result<!, ExitTaskError> {
    if System::is_interrupt_context() {
        return Err(ExitTaskError::BadContext);
    }

    // If CPU Lock is inactive, activate it.
    // TODO: If `is_cpu_lock_active() == true`, assert that it was an
    //       application that has the lock. It's illegal for it to be a
    //       kernel-owned CPU Lock.
    let mut lock = unsafe {
        if !System::is_cpu_lock_active() {
            System::enter_cpu_lock();
        }
        utils::assume_cpu_lock::<System>()
    };

    // If Priority Boost is active, deacrivate it.
    System::state()
        .priority_boost
        .store(false, Ordering::Release);

    // Transition the current task to Dormant
    let running_task = System::state().running_task().unwrap();
    assert_eq!(*running_task.st.read(&*lock), TaskSt::Running);
    running_task.st.replace(&mut *lock, TaskSt::Dormant);

    // Erase `running_task`
    System::state().running_task.store(None, Ordering::Relaxed);

    core::mem::forget(lock);

    // Safety: (1) The user of `exit_task` acknowledges that all preexisting
    // data on the task stack will be invalidated and has promised that this
    // will not cause any UBs. (2) CPU Lock active
    unsafe {
        System::exit_and_dispatch(running_task);
    }
}

/// Initialize a task at boot time.
pub(super) fn init_task<System: Kernel>(
    lock: utils::CpuLockGuardBorrowMut<'_, System>,
    task_cb: &'static TaskCb<System>,
) {
    if let TaskSt::PendingActivation = task_cb.st.read(&*lock) {
        // `PendingActivation` is equivalent to `Dormant` but serves as a marker
        // indicating tasks that should be activated by `init_task`.

        // Safety: CPU Lock active, the task is (essentially) in the Dormant state
        unsafe { System::initialize_task_state(task_cb) };

        // Safety: The previous state is PendingActivation (which is equivalent
        // to Dormant) and we just initialized the task state, so this is safe
        unsafe { make_ready(lock, task_cb) };
    }
}

/// Get a `ListAccessorCell` used to access a task ready queue.
macro_rules! list_accessor {
    (<$sys:ty>::state().task_ready_queue[$i:expr], $key:expr) => {
        ListAccessorCell::new(
            TaskReadyQueueHeadAccessor($i, &<$sys>::state().task_ready_queue),
            &Static,
            |task_cb: &TaskCb<$sys>| &task_cb.link,
            $key,
        )
    };
}

/// A helper type for `list_accessor`, implementing
/// `CellLike<StaticListHead<TaskCb<System>>>`.
struct TaskReadyQueueHeadAccessor<System: Port, TaskReadyQueue: 'static>(
    usize,
    &'static utils::CpuLockCell<System, TaskReadyQueue>,
);

impl<'a, System, TaskReadyQueue> CellLike<utils::CpuLockGuardBorrowMut<'a, System>>
    for TaskReadyQueueHeadAccessor<System, TaskReadyQueue>
where
    System: Kernel,
    TaskReadyQueue: core::borrow::BorrowMut<[StaticListHead<TaskCb<System>>]> + 'static,
{
    type Target = StaticListHead<TaskCb<System>>;

    fn get(&self, key: &utils::CpuLockGuardBorrowMut<'a, System>) -> Self::Target {
        self.1.read(&**key).borrow()[self.0]
    }
    fn set(&self, key: &mut utils::CpuLockGuardBorrowMut<'a, System>, value: Self::Target) {
        self.1.write(&mut **key).borrow_mut()[self.0] = value;
    }
}

/// Implements `Task::activate`.
fn activate<System: Kernel>(
    mut lock: utils::CpuLockGuard<System>,
    task_cb: &'static TaskCb<System>,
) -> Result<(), ActivateTaskError> {
    if *task_cb.st.read(&*lock) != TaskSt::Dormant {
        return Err(ActivateTaskError::QueueOverflow);
    }

    // Discard a park token if the task has one
    task_cb.park_token.replace(&mut *lock, false);

    // Safety: CPU Lock active, the task is in the Dormant state
    unsafe { System::initialize_task_state(task_cb) };

    // Safety: The previous state is Dormant, and we just initialized the task
    // state, so this is safe
    unsafe { make_ready(lock.borrow_mut(), task_cb) };

    // If `task_cb` has a higher priority, perform a context switch.
    unlock_cpu_and_check_preemption(lock);

    Ok(())
}

/// Transition the task into the Ready state. This function doesn't do any
/// proper cleanup for a previous state. If the previous state is `Dormant`, the
/// caller must initialize the task state first by calling
/// `initialize_task_state`.
pub(super) unsafe fn make_ready<System: Kernel>(
    mut lock: utils::CpuLockGuardBorrowMut<'_, System>,
    task_cb: &'static TaskCb<System>,
) {
    // Make the task Ready
    task_cb.st.replace(&mut *lock, TaskSt::Ready);

    // Insert the task to a ready queue
    let pri = task_cb.priority.to_usize().unwrap();
    list_accessor!(<System>::state().task_ready_queue[pri], lock.borrow_mut())
        .push_back(Ident(task_cb));

    // Update `task_ready_bitmap` accordingly
    <System>::state()
        .task_ready_bitmap
        .write(&mut *lock)
        .set(pri);
}

/// Relinquish CPU Lock. After that, if there's a higher-priority task than
/// `running_task`, call `Port::yield_cpu`.
///
/// System services that transition a task into the Ready state should call
/// this before returning to the caller.
pub(super) fn unlock_cpu_and_check_preemption<System: Kernel>(lock: utils::CpuLockGuard<System>) {
    // If Priority Boost is active, treat the currently running task as the
    // highest-priority task.
    if System::is_priority_boost_active() {
        debug_assert_eq!(
            *System::state().running_task().unwrap().st.read(&*lock),
            TaskSt::Running
        );
        return;
    }

    let prev_task_priority = if let Some(running_task) = System::state().running_task() {
        running_task.priority.to_usize().unwrap()
    } else {
        usize::max_value()
    };

    // The priority of the next task to run
    let next_task_priority = System::state()
        .task_ready_bitmap
        .read(&*lock)
        .find_set()
        .unwrap_or(usize::max_value());

    // Relinquish CPU Lock
    drop(lock);

    if next_task_priority < prev_task_priority {
        // Safety: CPU Lock inactive
        unsafe { System::yield_cpu() };
    }
}

/// Implements `PortToKernel::choose_running_task`.
pub(super) fn choose_next_running_task<System: Kernel>(
    mut lock: utils::CpuLockGuardBorrowMut<System>,
) {
    // If Priority Boost is active, treat the currently running task as the
    // highest-priority task.
    if System::is_priority_boost_active() {
        // Blocking system calls aren't allowed when Priority Boost is active
        debug_assert_eq!(
            *System::state().running_task().unwrap().st.read(&*lock),
            TaskSt::Running
        );
        return;
    }

    // The priority of `running_task`
    let prev_running_task = System::state().running_task();
    let prev_task_priority = if let Some(running_task) = prev_running_task {
        if *running_task.st.read(&*lock) == TaskSt::Running {
            running_task.priority.to_usize().unwrap()
        } else {
            usize::max_value()
        }
    } else {
        usize::max_value()
    };

    // The priority of the next task to run
    let next_task_priority = System::state()
        .task_ready_bitmap
        .read(&*lock)
        .find_set()
        .unwrap_or(usize::max_value());

    // Return if there's no task willing to take over the current one.
    if prev_task_priority <= next_task_priority {
        return;
    }

    // Find the next task to run
    let next_running_task = if next_task_priority < System::NUM_TASK_PRIORITY_LEVELS {
        // Take the first task in the ready queue for `next_task_priority`
        let mut accessor = list_accessor!(
            <System>::state().task_ready_queue[next_task_priority],
            lock.borrow_mut()
        );
        let task = accessor.pop_front().unwrap().0;

        // Update `task_ready_bitmap` accordingly
        if accessor.is_empty() {
            <System>::state()
                .task_ready_bitmap
                .write(&mut *lock)
                .clear(next_task_priority);
        }

        // Transition `next_running_task` into the Running state
        task.st.replace(&mut *lock, TaskSt::Running);

        Some(task)
    } else {
        None
    };

    // If `prev_running_task` is in the Running state, transition it into Ready
    if let Some(running_task) = prev_running_task {
        match running_task.st.read(&*lock) {
            TaskSt::Running => {
                // Safety: The previous state is Running, so this is safe
                unsafe { make_ready(lock.borrow_mut(), running_task) };
            }
            TaskSt::Waiting => {}
            _ => unreachable!(),
        }
    }

    System::state()
        .running_task
        .store(next_running_task, Ordering::Relaxed);
}

/// Transition the currently running task into the Waiting state. Returns when
/// woken up.
///
/// The current context must be [waitable] (This function doesn't check
/// that). The caller should use `expect_waitable_context` to do that.
///
/// [waitable]: crate#contets
pub(super) fn wait_until_woken_up<System: Kernel>(
    mut lock: utils::CpuLockGuardBorrowMut<'_, System>,
) {
    debug_assert_eq!(state::expect_waitable_context::<System>(), Ok(()));

    // Transition the current task to Waiting
    let running_task = System::state().running_task().unwrap();
    assert_eq!(*running_task.st.read(&*lock), TaskSt::Running);
    running_task.st.replace(&mut *lock, TaskSt::Waiting);

    loop {
        // Temporarily release the CPU Lock before calling `yield_cpu`
        // Safety: (1) We don't access rseources protected by CPU Lock.
        //         (2) We currently have CPU Lock.
        //         (3) We will re-acquire a CPU Lock before returning from this
        //             function.
        unsafe { System::leave_cpu_lock() };

        // Safety: CPU Lock inactive
        unsafe { System::yield_cpu() };

        // Re-acquire a CPU Lock
        unsafe { System::enter_cpu_lock() };

        if *running_task.st.read(&*lock) == TaskSt::Running {
            break;
        }
    }
}

/// Implements [`Kernel::park`].
pub(super) fn park_current_task<System: Kernel>() -> Result<(), ParkError> {
    let mut lock = utils::lock_cpu::<System>()?;
    state::expect_waitable_context::<System>()?;

    let running_task = System::state().running_task().unwrap();

    // If the task already has a park token, return immediately
    if running_task.park_token.replace(&mut *lock, false) {
        return Ok(());
    }

    // Wait until woken up by `unpark_exact`
    wait::wait_no_queue(lock.borrow_mut(), wait::WaitPayload::Park)?;

    Ok(())
}

/// Implements [`Task::unpark_exact`].
fn unpark_exact<System: Kernel>(
    mut lock: utils::CpuLockGuard<System>,
    task_cb: &'static TaskCb<System>,
) -> Result<(), UnparkExactError> {
    // Is the task currently parked?
    let is_parked = match task_cb.st.read(&*lock) {
        TaskSt::Dormant => return Err(UnparkExactError::BadObjectState),
        TaskSt::Waiting => wait::with_current_wait_payload(lock.borrow_mut(), task_cb, |payload| {
            matches!(payload, Some(wait::WaitPayload::Park))
        }),
        _ => false,
    };

    if is_parked {
        // Unblock the task. We confirmed that the task is in the Waiting state,
        // so `interrupt_task` should succeed.
        wait::interrupt_task(lock.borrow_mut(), task_cb, Ok(())).unwrap();

        // The task is now awake, check dispatch
        unlock_cpu_and_check_preemption(lock);

        Ok(())
    } else {
        // Put a park token
        if task_cb.park_token.replace(&mut *lock, true) {
            // It already had a park token
            Err(UnparkExactError::QueueOverflow)
        } else {
            Ok(())
        }
    }
}
