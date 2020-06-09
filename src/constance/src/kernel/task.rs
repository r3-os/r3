//! Tasks
use core::{cell::UnsafeCell, fmt, marker::PhantomData};

use super::{hunk::Hunk, utils, ActivateTaskError, ExitTaskError, Id, Kernel, KernelCfg1, Port};
use crate::utils::{Init, RawCell};

/// Represents a single task in a system.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Task<System>(Id, PhantomData<System>);

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
    pub const unsafe fn from_id(id: Id) -> Self {
        Self(id, PhantomData)
    }
}

impl<System: Kernel> Task<System> {
    /// Get the raw `Id` value representing this task.
    pub const fn id(self) -> Id {
        self.0
    }

    /// Start the execution of the task.
    pub fn activate(self) -> Result<(), ActivateTaskError> {
        let _lock = utils::lock_cpu::<System>()?;

        todo!()
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
    System: Port,
    PortTaskState = <System as Port>::PortTaskState,
    TaskPriority = <System as KernelCfg1>::TaskPriority,
> {
    /// Get a reference to `PortTaskState` in the task control block.
    ///
    /// This is guaranteed to be placed at the beginning of the struct so that
    /// assembler code can refer to this easily.
    pub port_task_state: PortTaskState,

    /// The static properties of the task.
    pub attr: &'static TaskAttr<System>,

    pub priority: TaskPriority,

    pub(super) _force_int_mut: RawCell<()>,
}

impl<System: Port, PortTaskState: Init, TaskPriority: Init> Init
    for TaskCb<System, PortTaskState, TaskPriority>
{
    const INIT: Self = Self {
        port_task_state: Init::INIT,
        attr: &TaskAttr::INIT,
        priority: Init::INIT,
        _force_int_mut: RawCell::new(()),
    };
}

impl<System: Kernel, PortTaskState: fmt::Debug, TaskPriority> fmt::Debug
    for TaskCb<System, PortTaskState, TaskPriority>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TaskCb")
            .field("port_task_state", &self.port_task_state)
            .field("attr", self.attr)
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

/// Implements [`Kernel::exit_task`].
pub(super) unsafe fn exit_current_task<System: Kernel>() -> Result<!, ExitTaskError> {
    // TODO: Deny interrupt context

    // If CPU Lock is inactive, activate it.
    // TODO: If `is_cpu_lock_active() == true`, assert that it was an
    //       application that has the lock. It's illegal for it to be a
    //       kernel-owned CPU Lock.
    let lock = unsafe {
        if !System::is_cpu_lock_active() {
            System::enter_cpu_lock();
        }
        utils::assume_cpu_lock::<System>()
    };

    // TODO: Transition the task to Dormant

    core::mem::forget(lock);

    // Safety: (1) The user of `exit_task` acknowledges that all preexisting
    // data on the task stack will be invalidated and has promised that this
    // will not cause any UBs. (2) CPU Lock active
    unsafe {
        System::exit_and_dispatch();
    }
}
