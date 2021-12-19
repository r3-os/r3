//! Tasks
#[cfg(feature = "priority_boost")]
use core::sync::atomic::Ordering;
use core::{fmt, marker::PhantomData, mem};
use num_traits::ToPrimitive;
use r3::{
    kernel::{
        raw::KernelBase, ActivateTaskError, ExitTaskError, GetCurrentTaskError,
        GetTaskPriorityError, Hunk, InterruptTaskError, ParkError, ParkTimeoutError,
        SetTaskPriorityError, SleepError, UnparkExactError, WaitTimeoutError,
    },
    time::Duration,
    utils::ConstDefault,
};

use crate::{
    error::NoAccessError, klock, mutex, state, timeout, wait, Id, KernelCfg1, KernelTraits,
    PortThreading, System,
};

#[doc(hidden)]
pub mod readyqueue;
use self::readyqueue::Queue as _;

pub(super) type TaskId = Id;

/// These associate functions implement the task-related portion of
/// [`r3::kernel::raw::KernelBase`].
impl<Traits: KernelTraits> System<Traits> {
    /// Get the [`TaskCb`] for the specified raw ID.
    ///
    /// # Safety
    ///
    /// See [`crate::bad_id`].
    #[inline]
    unsafe fn task_cb(this: TaskId) -> Result<&'static TaskCb<Traits>, NoAccessError> {
        Traits::get_task_cb(this.get() - 1).ok_or_else(|| unsafe { crate::bad_id::<Traits>() })
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    pub(super) fn task_current() -> Result<TaskId, GetCurrentTaskError> {
        if !Traits::is_task_context() {
            return Err(GetCurrentTaskError::BadContext);
        }

        let mut lock = klock::lock_cpu::<Traits>()?;
        let task_cb = Traits::state().running_task(lock.borrow_mut()).unwrap();

        // Calculate an `Id` from the task CB pointer
        let offset_bytes =
            task_cb as *const TaskCb<_> as usize - Traits::task_cb_pool().as_ptr() as usize;
        let offset = offset_bytes / mem::size_of::<TaskCb<Traits>>();

        let task = Id::new(offset as usize + 1).unwrap();

        Ok(task)
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    pub(super) fn task_activate(this: TaskId) -> Result<(), ActivateTaskError> {
        let lock = klock::lock_cpu::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let task_cb = unsafe { Self::task_cb(this)? };
        activate(lock, task_cb)
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    pub(super) fn task_interrupt(this: TaskId) -> Result<(), InterruptTaskError> {
        let mut lock = klock::lock_cpu::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let task_cb = unsafe { Self::task_cb(this)? };
        wait::interrupt_task(
            lock.borrow_mut(),
            task_cb,
            Err(WaitTimeoutError::Interrupted),
        )?;

        // The task is now awake, check dispatch
        unlock_cpu_and_check_preemption(lock);

        Ok(())
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    pub(super) fn task_unpark_exact(this: TaskId) -> Result<(), UnparkExactError> {
        let lock = klock::lock_cpu::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let task_cb = unsafe { Self::task_cb(this)? };
        unpark_exact(lock, task_cb)
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    pub(super) fn task_set_priority(
        this: TaskId,
        priority: usize,
    ) -> Result<(), SetTaskPriorityError> {
        let lock = klock::lock_cpu::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let task_cb = unsafe { Self::task_cb(this)? };
        set_task_base_priority(lock, task_cb, priority)
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    pub(super) fn task_priority(this: TaskId) -> Result<usize, GetTaskPriorityError> {
        let lock = klock::lock_cpu::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let task_cb = unsafe { Self::task_cb(this)? };

        if *task_cb.st.read(&*lock) == TaskSt::Dormant {
            Err(GetTaskPriorityError::BadObjectState)
        } else {
            Ok(task_cb.base_priority.read(&*lock).to_usize().unwrap())
        }
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    pub(super) fn task_effective_priority(this: TaskId) -> Result<usize, GetTaskPriorityError> {
        let lock = klock::lock_cpu::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let task_cb = unsafe { Self::task_cb(this)? };

        if *task_cb.st.read(&*lock) == TaskSt::Dormant {
            Err(GetTaskPriorityError::BadObjectState)
        } else {
            Ok(task_cb.effective_priority.read(&*lock).to_usize().unwrap())
        }
    }
}

// FIXME: Since we don't want to say "task stack is guaranteed to be a hunk" in
//        a public interface, we should rename this type
/// [`Hunk`] for a task stack.
pub struct StackHunk<Traits> {
    _phantom: PhantomData<Traits>,
    hunk_offset: usize,
    len: usize,
}

const STACK_HUNK_AUTO: usize = (isize::MIN) as usize;

// Safety: Safe code can't access the contents. Also, the port is responsible
// for making sure `StackHunk` is used in the correct way.
unsafe impl<Traits: KernelTraits> Sync for StackHunk<Traits> {}

impl<Traits: KernelTraits> fmt::Debug for StackHunk<Traits> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("StackHunk")
            .field(&self.hunk().as_ptr())
            .finish()
    }
}

impl<Traits: KernelTraits> Clone for StackHunk<Traits> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<Traits: KernelTraits> Copy for StackHunk<Traits> {}

// TODO: Should we allow zero-sized `StackHunk`?
impl<Traits: KernelTraits> ConstDefault for StackHunk<Traits> {
    const DEFAULT: Self = Self {
        _phantom: PhantomData,
        hunk_offset: 0,
        len: 0,
    };
}

impl<Traits: KernelTraits> StackHunk<Traits> {
    #[inline]
    const fn hunk(&self) -> Hunk<System<Traits>> {
        // FIXME: `Hunk::from_offset` is an implementation detail
        Hunk::from_offset(self.hunk_offset)
    }

    /// Construct a `StackHunk` from `Hunk`.
    pub(crate) const fn from_hunk(hunk: Hunk<System<Traits>>, len: usize) -> Self {
        assert!(len & STACK_HUNK_AUTO == 0, "too large");
        Self {
            _phantom: PhantomData,
            hunk_offset: hunk.offset(),
            len,
        }
    }

    /// Construct a `StackHunk` representing an automatically allocated stack
    /// region.
    ///
    /// `StackHunk`s created by this method are supposed to be converted to
    /// non-automatic `StackHunk`s during the configuration phase.
    pub(crate) const fn auto(len: usize) -> Self {
        assert!(len & STACK_HUNK_AUTO == 0, "too large");
        Self {
            _phantom: PhantomData,
            hunk_offset: 0,
            len: len | STACK_HUNK_AUTO,
        }
    }

    /// Get the requested size if this `StackHunk` represents an automatically
    /// allocated stack region.
    pub(crate) const fn auto_size(self) -> Option<usize> {
        if self.len & STACK_HUNK_AUTO != 0 {
            Some(self.len & !STACK_HUNK_AUTO)
        } else {
            None
        }
    }
}

impl<Traits: KernelTraits> StackHunk<Traits> {
    /// Get a raw pointer to the hunk's contents.
    ///
    /// This is mainly used by [`PortThreading::initialize_task_state`] to
    /// calculate the initial stack pointer.
    #[inline]
    pub fn as_ptr(&self) -> *mut [u8] {
        core::ptr::slice_from_raw_parts_mut(self.hunk().as_ptr(), self.len)
    }
}

/// *Task control block* - the state data of a task.
#[repr(C)]
pub struct TaskCb<
    Traits: PortThreading,
    PortTaskState: 'static = <Traits as PortThreading>::PortTaskState,
    TaskPriority: 'static = <Traits as KernelCfg1>::TaskPriority,
    TaskReadyQueueData: 'static = <<Traits as KernelCfg1>::TaskReadyQueue as readyqueue::Queue<
        Traits,
    >>::PerTaskData,
> {
    /// Get a reference to `PortTaskState` in the task control block.
    ///
    /// This is guaranteed to be placed at the beginning of the struct so that
    /// assembler code can refer to this easily.
    pub port_task_state: PortTaskState,

    /// The static properties of the task.
    pub attr: &'static TaskAttr<Traits, TaskPriority>,

    /// The task's base priority.
    pub(super) base_priority: klock::CpuLockCell<Traits, TaskPriority>,

    /// The task's effective priority. It's calculated based on `base_priority`
    /// and may be temporarily elevated by a mutex locking protocol.
    ///
    /// Given a set of mutexes held by the task `mutexes`, the value is
    /// calculated by the following pseudocode:
    ///
    /// ```rust,ignore
    /// task_cb.base_priority.min(mutexes.map(|mutex_cb| {
    ///     if let Some(ceiling) = mutex_cb.ceiling {
    ///         assert!(ceiling <= task_cb.base_priority);
    ///         ceiling
    ///     } else {
    ///         TaskPriority::MAX
    ///     }
    /// }).min())
    /// ```
    ///
    /// Many operations change the inputs of this calculation. We take care to
    /// ensure the recalculation of this value completes in constant-time (in
    /// regard to the number of held mutexes) for as many cases as possible.
    ///
    /// The effective priority determines the task's position within the task
    /// ready queue. You must call `TaskReadyQueue::reorder_task` after updating
    /// `effective_priority` of a task which is in Ready state.
    pub(super) effective_priority: klock::CpuLockCell<Traits, TaskPriority>,

    pub(super) st: klock::CpuLockCell<Traits, TaskSt>,

    /// A flag indicating whether the task has a park token or not.
    pub(super) park_token: klock::CpuLockCell<Traits, bool>,

    /// Allows `TaskCb` to participate in one of linked lists.
    ///
    ///  - In a `Ready` state, this forms the linked list headed by
    ///    [`State::task_ready_queue`].
    ///
    /// [`State::task_ready_queue`]: crate::State::task_ready_queue
    pub(super) ready_queue_data: TaskReadyQueueData,

    /// The wait state of the task.
    pub(super) wait: wait::TaskWait<Traits>,

    /// The last mutex locked by the task.
    pub(super) last_mutex_held: klock::CpuLockCell<Traits, Option<&'static mutex::MutexCb<Traits>>>,
}

impl<
        Traits: KernelTraits,
        PortTaskState: fmt::Debug + 'static,
        TaskPriority: fmt::Debug + 'static,
        TaskReadyQueueData: fmt::Debug + 'static,
    > fmt::Debug for TaskCb<Traits, PortTaskState, TaskPriority, TaskReadyQueueData>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TaskCb")
            .field("self", &(self as *const _))
            .field("port_task_state", &self.port_task_state)
            .field("attr", self.attr)
            .field("base_priority", &self.base_priority)
            .field("effective_priority", &self.effective_priority)
            .field("st", &self.st)
            .field("ready_queue_data", &self.ready_queue_data)
            .field("wait", &self.wait)
            .field(
                "last_mutex_held",
                // Don't print the content of the mutex. It'll be printed
                // somewhere else in the debug printing of `KernelDebugPrinter`.
                &self
                    .last_mutex_held
                    .debug_fmt_with(|x, f| x.map(|x| x as *const _).fmt(f)),
            )
            .field("park_token", &self.park_token)
            .finish()
    }
}

/// The static properties of a task.
pub struct TaskAttr<
    Traits: KernelCfg1,
    TaskPriority: 'static = <Traits as KernelCfg1>::TaskPriority,
> {
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
    pub stack: StackHunk<Traits>,

    /// The initial base priority of the task.
    pub priority: TaskPriority,
}

impl<Traits: KernelTraits, TaskPriority: fmt::Debug> fmt::Debug for TaskAttr<Traits, TaskPriority> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TaskAttr")
            .field("entry_point", &self.entry_point)
            .field("entry_param", &self.entry_param)
            .field("stack", &self.stack)
            .field("priority", &self.priority)
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

impl ConstDefault for TaskSt {
    const DEFAULT: Self = Self::Dormant;
}

/// Implements `KernelBase::exit_task`.
pub(super) unsafe fn exit_current_task<Traits: KernelTraits>() -> Result<!, ExitTaskError> {
    if !Traits::is_task_context() {
        return Err(ExitTaskError::BadContext);
    }

    // If CPU Lock is inactive, activate it.
    // TODO: If `is_cpu_lock_active() == true`, assert that it was an
    //       application that has the lock. It's illegal for it to be a
    //       kernel-owned CPU Lock.
    let mut lock = unsafe {
        if !Traits::is_cpu_lock_active() {
            Traits::enter_cpu_lock();
        }
        klock::assume_cpu_lock::<Traits>()
    };

    #[cfg(feature = "priority_boost")]
    {
        // If Priority Boost is active, deactivate it.
        Traits::state()
            .priority_boost
            .store(false, Ordering::Release);
    }

    let running_task = Traits::state().running_task(lock.borrow_mut()).unwrap();

    // Abandon mutexes, waking up the next waiters of the mutexes (if any)
    mutex::abandon_held_mutexes(lock.borrow_mut(), running_task);
    debug_assert!(running_task.last_mutex_held.read(&*lock).is_none());

    // Transition the current task to Dormant
    assert_eq!(*running_task.st.read(&*lock), TaskSt::Running);
    running_task.st.replace(&mut *lock, TaskSt::Dormant);

    // Erase `running_task`
    Traits::state().running_task.replace(&mut *lock, None);

    core::mem::forget(lock);

    // Safety: (1) The user of `exit_task` acknowledges that all preexisting
    // data on the task stack will be invalidated and has promised that this
    // will not cause any UBs. (2) CPU Lock active
    unsafe {
        Traits::exit_and_dispatch(running_task);
    }
}

/// Initialize a task at boot time.
pub(super) fn init_task<Traits: KernelTraits>(
    lock: klock::CpuLockTokenRefMut<'_, Traits>,
    task_cb: &'static TaskCb<Traits>,
) {
    if let TaskSt::PendingActivation = task_cb.st.read(&*lock) {
        // `PendingActivation` is equivalent to `Dormant` but serves as a marker
        // indicating tasks that should be activated by `init_task`.

        // Safety: CPU Lock active, the task is (essentially) in the Dormant state
        unsafe { Traits::initialize_task_state(task_cb) };

        // Safety: The previous state is PendingActivation (which is equivalent
        // to Dormant) and we just initialized the task state, so this is safe
        unsafe { make_ready(lock, task_cb) };
    }
}

/// Implements `Task::activate`.
fn activate<Traits: KernelTraits>(
    mut lock: klock::CpuLockGuard<Traits>,
    task_cb: &'static TaskCb<Traits>,
) -> Result<(), ActivateTaskError> {
    if *task_cb.st.read(&*lock) != TaskSt::Dormant {
        return Err(ActivateTaskError::QueueOverflow);
    }

    // Discard a park token if the task has one
    task_cb.park_token.replace(&mut *lock, false);

    // Safety: CPU Lock active, the task is in the Dormant state
    unsafe { Traits::initialize_task_state(task_cb) };

    // Reset the task priority
    task_cb
        .base_priority
        .replace(&mut *lock, task_cb.attr.priority);
    task_cb
        .effective_priority
        .replace(&mut *lock, task_cb.attr.priority);

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
pub(super) unsafe fn make_ready<Traits: KernelTraits>(
    mut lock: klock::CpuLockTokenRefMut<'_, Traits>,
    task_cb: &'static TaskCb<Traits>,
) {
    // Make the task Ready
    task_cb.st.replace(&mut *lock, TaskSt::Ready);

    // Insert the task to the ready queue.
    //
    // Safety: `task_cb` is not in the ready queue
    unsafe {
        <Traits>::state()
            .task_ready_queue
            .push_back_task(lock.into(), task_cb);
    }
}

/// Relinquish CPU Lock. After that, if there's a higher-priority task than
/// `running_task`, call `Port::yield_cpu`.
///
/// System services that transition a task into the Ready state should call
/// this before returning to the caller.
pub(super) fn unlock_cpu_and_check_preemption<Traits: KernelTraits>(
    mut lock: klock::CpuLockGuard<Traits>,
) {
    // If Priority Boost is active, treat the currently running task as the
    // highest-priority task.
    if System::<Traits>::raw_is_priority_boost_active() {
        debug_assert_eq!(
            *Traits::state()
                .running_task(lock.borrow_mut())
                .unwrap()
                .st
                .read(&*lock),
            TaskSt::Running
        );
        return;
    }

    let prev_task_priority =
        if let Some(running_task) = Traits::state().running_task(lock.borrow_mut()) {
            if *running_task.st.read(&*lock) == TaskSt::Running {
                running_task
                    .effective_priority
                    .read(&*lock)
                    .to_usize()
                    .unwrap()
            } else {
                usize::MAX
            }
        } else {
            usize::MAX
        };

    let has_preempting_task = Traits::state()
        .task_ready_queue
        .has_ready_task_in_priority_range(lock.borrow_mut().into(), ..prev_task_priority);

    // Relinquish CPU Lock
    drop(lock);

    if has_preempting_task {
        // Safety: CPU Lock inactive
        unsafe { Traits::yield_cpu() };
    }
}

/// Implements `PortToKernel::choose_running_task`.
#[inline]
pub(super) fn choose_next_running_task<Traits: KernelTraits>(
    mut lock: klock::CpuLockTokenRefMut<Traits>,
) {
    // If Priority Boost is active, treat the currently running task as the
    // highest-priority task.
    if System::<Traits>::raw_is_priority_boost_active() {
        // Blocking system calls aren't allowed when Priority Boost is active
        debug_assert_eq!(
            *Traits::state()
                .running_task(lock.borrow_mut())
                .unwrap()
                .st
                .read(&*lock),
            TaskSt::Running
        );
        return;
    }

    // The priority of `running_task`
    let prev_running_task = Traits::state().running_task(lock.borrow_mut());
    let prev_task_priority = if let Some(running_task) = prev_running_task {
        if *running_task.st.read(&*lock) == TaskSt::Running {
            running_task
                .effective_priority
                .read(&*lock)
                .to_usize()
                .unwrap()
        } else {
            usize::MAX // (2) see the discussion below
        }
    } else {
        usize::MAX // (1) see the discussion below
    };

    // Decide the next task to run
    //
    // The special value `prev_task_priority == usize::MAX` indicates that
    // (1) there is no running task, or (2) there was one but it is not running
    // anymore, and we need to elect a new task to run. In case (2), we would
    // want to update `running_task` regardless of whether there exists a
    // schedulable task or not. That is, even if there was not such a task, we
    // would still want to assign `None` to `running_task`. Therefore,
    // `pop_front_task` is designed to return `SwitchTo(None)` in this case.
    let decision = Traits::state()
        .task_ready_queue
        .pop_front_task(lock.borrow_mut().into(), prev_task_priority);

    let next_running_task = match decision {
        readyqueue::ScheduleDecision::SwitchTo(task) => task,

        // Return if there's no task willing to take over the current one, and
        // the current one can still run.
        readyqueue::ScheduleDecision::Keep => {
            // If `prev_task_priority == usize::MAX`, `pop_front_task` must
            // return `SwitchTo(_)`.
            debug_assert_ne!(prev_task_priority, usize::MAX);
            return;
        }
    };

    if let Some(task) = next_running_task {
        // Transition `next_running_task` into the Running state
        task.st.replace(&mut *lock, TaskSt::Running);

        if ptr_from_option_ref(prev_running_task) == task {
            // Skip the remaining steps if `task == prev_running_task`
            return;
        }
    }

    // `prev_running_task` now loses the control of the processor.
    if let Some(running_task) = prev_running_task {
        debug_assert_ne!(
            ptr_from_option_ref(prev_running_task),
            ptr_from_option_ref(next_running_task),
        );
        match running_task.st.read(&*lock) {
            TaskSt::Running => {
                // Transition `prev_running_task` into Ready state.
                // Safety: The previous state is Running, so this is safe
                unsafe { make_ready(lock.borrow_mut(), running_task) };
            }
            TaskSt::Waiting => {
                // `prev_running_task` stays in Waiting state.
            }
            TaskSt::Ready => {
                // `prev_running_task` stays in Ready state.
            }
            _ => unreachable!(),
        }
    }

    Traits::state()
        .running_task
        .replace(&mut *lock, next_running_task);
}

#[inline]
fn ptr_from_option_ref<T>(x: Option<&T>) -> *const T {
    if let Some(x) = x {
        x
    } else {
        core::ptr::null()
    }
}

/// Transition the currently running task into the Waiting state. Returns when
/// woken up.
///
/// The current context must be [waitable] (This function doesn't check
/// that). The caller should use `expect_waitable_context` to do that.
///
/// [waitable]: crate#contets
pub(super) fn wait_until_woken_up<Traits: KernelTraits>(
    mut lock: klock::CpuLockTokenRefMut<'_, Traits>,
) {
    debug_assert_eq!(state::expect_waitable_context::<Traits>(), Ok(()));

    // Transition the current task to Waiting
    let running_task = Traits::state().running_task(lock.borrow_mut()).unwrap();
    assert_eq!(*running_task.st.read(&*lock), TaskSt::Running);
    running_task.st.replace(&mut *lock, TaskSt::Waiting);

    loop {
        // Temporarily release the CPU Lock before calling `yield_cpu`
        // Safety: (1) We don't access rseources protected by CPU Lock.
        //         (2) We currently have CPU Lock.
        //         (3) We will re-acquire a CPU Lock before returning from this
        //             function.
        unsafe { Traits::leave_cpu_lock() };

        // Safety: CPU Lock inactive
        unsafe { Traits::yield_cpu() };

        // Re-acquire a CPU Lock
        unsafe { Traits::enter_cpu_lock() };

        if *running_task.st.read(&*lock) == TaskSt::Running {
            break;
        }

        assert_eq!(*running_task.st.read(&*lock), TaskSt::Waiting);
    }
}

/// Implements `KernelBase::park`.
pub(super) fn park_current_task<Traits: KernelTraits>() -> Result<(), ParkError> {
    let mut lock = klock::lock_cpu::<Traits>()?;
    state::expect_waitable_context::<Traits>()?;

    let running_task = Traits::state().running_task(lock.borrow_mut()).unwrap();

    // If the task already has a park token, return immediately
    if running_task.park_token.replace(&mut *lock, false) {
        return Ok(());
    }

    // Wait until woken up by `unpark_exact`
    wait::wait_no_queue(lock.borrow_mut(), wait::WaitPayload::Park)?;

    Ok(())
}

/// Implements `KernelBase::park_timeout`.
pub(super) fn park_current_task_timeout<Traits: KernelTraits>(
    timeout: Duration,
) -> Result<(), ParkTimeoutError> {
    let time32 = timeout::time32_from_duration(timeout)?;
    let mut lock = klock::lock_cpu::<Traits>()?;
    state::expect_waitable_context::<Traits>()?;

    let running_task = Traits::state().running_task(lock.borrow_mut()).unwrap();

    // If the task already has a park token, return immediately
    if running_task.park_token.replace(&mut *lock, false) {
        return Ok(());
    }

    // Wait until woken up by `unpark_exact`
    wait::wait_no_queue_timeout(lock.borrow_mut(), wait::WaitPayload::Park, time32)?;

    Ok(())
}

/// Implements [`Task::unpark_exact`].
fn unpark_exact<Traits: KernelTraits>(
    mut lock: klock::CpuLockGuard<Traits>,
    task_cb: &'static TaskCb<Traits>,
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

/// Implements `KernelBase::sleep`.
pub(super) fn put_current_task_on_sleep_timeout<Traits: KernelTraits>(
    timeout: Duration,
) -> Result<(), SleepError> {
    let time32 = timeout::time32_from_duration(timeout)?;
    let mut lock = klock::lock_cpu::<Traits>()?;
    state::expect_waitable_context::<Traits>()?;

    // Wait until woken up by timeout
    match wait::wait_no_queue_timeout(lock.borrow_mut(), wait::WaitPayload::Sleep, time32) {
        Ok(_) => unreachable!(),
        Err(WaitTimeoutError::Interrupted) => Err(SleepError::Interrupted),
        Err(WaitTimeoutError::Timeout) => Ok(()),
    }
}

/// Implements [`Task::set_priority`].
fn set_task_base_priority<Traits: KernelTraits>(
    mut lock: klock::CpuLockGuard<Traits>,
    task_cb: &'static TaskCb<Traits>,
    base_priority: usize,
) -> Result<(), SetTaskPriorityError> {
    // Validate the given priority
    if base_priority >= Traits::NUM_TASK_PRIORITY_LEVELS {
        return Err(SetTaskPriorityError::BadParam);
    }
    let base_priority_internal =
        Traits::TaskPriority::try_from(base_priority).unwrap_or_else(|_| unreachable!());

    let st = *task_cb.st.read(&*lock);

    if st == TaskSt::Dormant {
        return Err(SetTaskPriorityError::BadObjectState);
    }

    let old_base_priority = task_cb.base_priority.read(&*lock).to_usize().unwrap();

    if old_base_priority == base_priority {
        return Ok(());
    }

    // Fail with `BadParam` if the operation would violate the precondition of
    // the locking protocol used in any of the held or waited mutexes. This
    // check is only needed when raising the priority.
    if base_priority < old_base_priority {
        // Get the currently-waited mutex (if any).
        let waited_mutex = wait::with_current_wait_payload(lock.borrow_mut(), task_cb, |payload| {
            if let Some(&wait::WaitPayload::Mutex(mutex_cb)) = payload {
                Some(mutex_cb)
            } else {
                None
            }
        });

        if let Some(waited_mutex) = waited_mutex {
            if !mutex::does_held_mutex_allow_new_task_base_priority(
                lock.borrow_mut(),
                waited_mutex,
                base_priority_internal,
            ) {
                return Err(SetTaskPriorityError::BadParam);
            }
        }

        // Check the precondition for all currently-held mutexes
        if !mutex::do_held_mutexes_allow_new_task_base_priority(
            lock.borrow_mut(),
            task_cb,
            base_priority_internal,
        ) {
            return Err(SetTaskPriorityError::BadParam);
        }
    }

    // Recalculate `effective_priority` according to the locking protocol
    // of held mutexes
    let effective_priority_internal =
        mutex::evaluate_task_effective_priority(lock.borrow_mut(), task_cb, base_priority_internal);
    let effective_priority = effective_priority_internal.to_usize().unwrap();

    // Assign the new priority
    task_cb
        .base_priority
        .replace(&mut *lock, base_priority_internal);
    let old_effective_priority = task_cb
        .effective_priority
        .replace(&mut *lock, effective_priority_internal)
        .to_usize()
        .unwrap();

    if old_effective_priority == effective_priority {
        return Ok(());
    }

    match st {
        TaskSt::Ready => unsafe {
            // Move the task within the ready queue
            //
            // Safety: `task_cb` was previously inserted to the ready queue
            // with an effective priority that is identical to
            // `old_effective_priority`.
            Traits::state().task_ready_queue.reorder_task(
                lock.borrow_mut().into(),
                task_cb,
                effective_priority,
                old_effective_priority,
            );
        },
        TaskSt::Running => {}
        TaskSt::Waiting => {
            // Reposition the task in a wait queue if the task is currently waiting
            wait::reorder_wait_of_task(lock.borrow_mut(), task_cb);
        }
        TaskSt::Dormant | TaskSt::PendingActivation => unreachable!(),
    }

    if let TaskSt::Running | TaskSt::Ready = st {
        // - If `st == TaskSt::Running`, `task_cb` is the currently running
        //   task. If the priority was lowered, it could be preempted by
        //   a task in the Ready state.
        // - If `st == TaskSt::Ready` and the priority was raised, it could
        //   preempt the currently running task.
        unlock_cpu_and_check_preemption(lock);
    }

    Ok(())
}
