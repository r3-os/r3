//! The RTOS kernel
use atomic_ref::AtomicRef;
use core::{borrow::BorrowMut, fmt, mem::forget, num::NonZeroUsize, sync::atomic::Ordering};

use crate::utils::{intrusive_list::StaticListHead, BinUInteger, Init, PrioBitmap};

#[macro_use]
mod cfg;
mod error;
mod hunk;
mod task;
mod utils;
pub use self::{cfg::*, error::*, hunk::*, task::*};

/// Numeric value used to identify various kinds of kernel objects.
pub type Id = NonZeroUsize;

/// Provides access to the global API functions exposed by the kernel.
///
/// This trait is automatically implemented on "system" types that have
/// sufficient trait `impl`s to instantiate the kernel.
pub trait Kernel: Port + KernelCfg2 + Sized + 'static {
    /// Terminate the current task, putting it into a Dormant state.
    ///
    /// The kernel (to be precise, the port) makes an implicit call to this
    /// function when a task entry point function returns.
    ///
    /// # Safety
    ///
    /// On a successful call, this function destroys the current task's stack
    /// without running any destructors on stack-allocated objects and renders
    /// all references pointing to such objects invalid. The caller is
    /// responsible for taking this possibility into account and ensuring this
    /// doesn't lead to an undefined behavior.
    ///
    unsafe fn exit_task() -> Result<!, ExitTaskError>;
}

impl<T: Port + KernelCfg2 + 'static> Kernel for T {
    unsafe fn exit_task() -> Result<!, ExitTaskError> {
        // Safety: Just forwarding the function call
        unsafe { exit_current_task::<Self>() }
    }
}

/// Associates "system" types with kernel-private data. Use [`build!`] to
/// implement.
///
/// Customizable things needed by both of `Port` and `KernelCfg2` should live
/// here because `Port` cannot refer to an associated item defined by
/// `KernelCfg2`.
///
/// # Safety
///
/// This is only intended to be implemented by `build!`.
pub unsafe trait KernelCfg1: Sized + 'static {
    /// The number of task priority levels.
    const NUM_TASK_PRIORITY_LEVELS: usize;

    /// Unsigned integer type capable of representing the range
    /// `0..NUM_TASK_PRIORITY_LEVELS`.
    type TaskPriority: BinUInteger;

    // FIXME: This is a work-around for trait methods being uncallable in `const fn`
    //        <https://github.com/rust-lang/rfcs/pull/2632>
    //        <https://github.com/rust-lang/const-eval/pull/8>
    /// All possible values of `TaskPriority`.
    ///
    /// `TASK_PRIORITY_LEVELS[i]` is equivalent to
    /// `TaskPriority::try_from(i).unwrap()` except that the latter doesn't work
    /// in `const fn`.
    const TASK_PRIORITY_LEVELS: &'static [Self::TaskPriority];
}

/// Implemented by a port.
///
/// # Safety
///
/// Implementing a port is inherently unsafe because it's responsible for
/// initializing the execution environment and providing a dispatcher
/// implementation.
///
/// These methods are only meant to be called by the kernel.
#[allow(clippy::missing_safety_doc)]
pub unsafe trait Port: KernelCfg1 {
    type PortTaskState: Send + Sync + Init + 'static;

    /// The initial value of [`TaskCb::port_task_state`] for all tasks.
    #[allow(clippy::declare_interior_mutable_const)] // it's intentional
    const PORT_TASK_STATE_INIT: Self::PortTaskState;

    /// The default stack size for tasks.
    const STACK_DEFAULT_SIZE: usize = 1024;

    /// The alignment requirement for task stack regions.
    const STACK_ALIGN: usize = core::mem::size_of::<usize>();

    /// Transfer the control to [`State::running_task`], discarding the current
    /// (startup) context.
    ///
    /// Precondition: CPU Lock active, Startup phase
    unsafe fn dispatch_first_task() -> !;

    /// Yield the processor.
    ///
    /// Precondition: CPU Lock inactive
    unsafe fn yield_cpu();

    /// Destroy the state of the currently running task
    /// ([`State::running_task`]) and proceed to the dispatcher.
    ///
    /// Precondition: CPU Lock active, Task context
    unsafe fn exit_and_dispatch() -> !;

    /// Disable all kernel-managed interrupts (this state is called *CPU Lock*).
    ///
    /// Precondition: CPU Lock inactive
    unsafe fn enter_cpu_lock();

    /// Re-enable kernel-managed interrupts previously disabled by
    /// `enter_cpu_lock`, thus deactivating the CPU Lock state.
    ///
    /// Precondition: CPU Lock active
    unsafe fn leave_cpu_lock();

    /// Prepare the task for activation. More specifically, set the current
    /// program counter to [`TaskAttr::entry_point`] and the current stack
    /// pointer to either end of [`TaskAttr::stack`], ensuring the task will
    /// start execution from `entry_point` next time the task receives the
    /// control.
    ///
    /// Do not call this for a running task. Calling this for a dormant task is
    /// always safe. For tasks in other states, whether this method is safe is
    /// dependent on how the programming language the task code is written in
    /// is implemented. In particular, this is unsafe for Rust task code because
    /// it might violate the requirement of [`Pin`] if there's a `Pin` pointing
    /// to something on the task's stack.
    ///
    /// [`Pin`]: core::pin::Pin
    ///
    /// Precondition: CPU Lock active
    unsafe fn initialize_task_state(task: &'static task::TaskCb<Self>);

    /// Return a flag indicating whether a CPU Lock state is active.
    fn is_cpu_lock_active() -> bool;
}

/// Methods intended to be called by a port.
///
/// # Safety
///
/// These are only meant to be called by the port.
#[allow(clippy::missing_safety_doc)]
pub trait PortToKernel {
    /// Initialize runtime structures.
    ///
    /// Should be called for exactly once by the port before calling into any
    /// user (application) or kernel code.
    ///
    /// Precondition: CPU Lock active, Preboot phase
    // TODO: Explain phases
    unsafe fn boot() -> !;

    /// Determine the next task to run and store it in [`State::active_task_ref`].
    ///
    /// Precondition: CPU Lock active / Postcondition: CPU Lock active
    unsafe fn choose_running_task();
}

impl<System: Kernel> PortToKernel for System {
    unsafe fn boot() -> ! {
        // Safety: (1) User code hasn't executed yet at this point. (2) The
        // creator of this `HunkAttr` is responsible for creating a valid
        // instance of `HunkAttr`.
        unsafe {
            System::HUNK_ATTR.init_hunks();
        }

        // Initialize all tasks
        // TODO: Do this only for initially-active tasks
        for cb in Self::task_cb_pool() {
            // Safety: The task is dormant
            unsafe {
                Self::initialize_task_state(cb);
            }
        }

        // Safety: CPU Lock is active, Startup phase
        unsafe {
            Self::dispatch_first_task();
        }
    }

    unsafe fn choose_running_task() {
        // Safety: The precondition of this method includes CPU Lock being
        // active
        let lock = unsafe { utils::assume_cpu_lock::<Self>() };

        // TODO: Choose only a runnable task
        Self::state()
            .running_task
            .store(Self::get_task_cb(0), Ordering::Relaxed);

        // Post-condition: CPU Lock active
        forget(lock);
    }
}

/// Associates "system" types with kernel-private data. Use [`build!`] to
/// implement.
///
/// # Safety
///
/// This is only intended to be implemented by `build!`.
pub unsafe trait KernelCfg2: Port + Sized {
    #[doc(hidden)]
    const HUNK_ATTR: HunkAttr;

    type TaskReadyBitmap: PrioBitmap;

    type TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<Self>>]> + Init + 'static;

    /// Access the kernel's global state.
    fn state() -> &'static State<Self>;

    // FIXME: Waiting for <https://github.com/rust-lang/const-eval/issues/11>
    //        to be resolved because `TaskCb` includes interior mutability
    //        and can't be referred to by `const`
    #[doc(hidden)]
    fn task_cb_pool() -> &'static [TaskCb<Self>];

    #[doc(hidden)]
    #[inline(always)]
    fn get_task_cb(i: usize) -> Option<&'static TaskCb<Self>> {
        Self::task_cb_pool().get(i)
    }
}

/// Global kernel state.
pub struct State<
    System: KernelCfg2,
    PortTaskState: 'static = <System as Port>::PortTaskState,
    TaskReadyBitmap: PrioBitmap = <System as KernelCfg2>::TaskReadyBitmap,
    TaskReadyQueue: 'static = <System as KernelCfg2>::TaskReadyQueue,
    TaskPriority: 'static = <System as KernelCfg1>::TaskPriority,
> {
    // TODO: Make `running_task` non-null to simplify runtime code
    /// The currently running task.
    running_task: AtomicRef<'static, TaskCb<System, PortTaskState, TaskPriority>>,

    /// The task ready bitmap, in which each bit indicates whether the
    /// task ready queue corresponding to that bit contains a task or not.
    task_ready_bitmap: utils::CpuLockCell<System, TaskReadyBitmap>,

    /// The task ready queues, in which each queue represents the list of
    /// runnable task at the corresponding priority level.
    task_ready_queue: utils::CpuLockCell<System, TaskReadyQueue>,
}

impl<
        System: KernelCfg2,
        PortTaskState: 'static,
        TaskReadyBitmap: PrioBitmap,
        TaskReadyQueue: 'static + Init,
        TaskPriority: 'static,
    > Init for State<System, PortTaskState, TaskReadyBitmap, TaskReadyQueue, TaskPriority>
{
    const INIT: Self = Self {
        running_task: AtomicRef::new(None),
        task_ready_bitmap: Init::INIT,
        task_ready_queue: Init::INIT,
    };
}

impl<
        System: Kernel,
        PortTaskState: 'static + fmt::Debug,
        TaskReadyBitmap: PrioBitmap,
        TaskReadyQueue: 'static + fmt::Debug,
        TaskPriority: 'static,
    > fmt::Debug for State<System, PortTaskState, TaskReadyBitmap, TaskReadyQueue, TaskPriority>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("State")
            .field("running_task", &self.running_task)
            .field("task_ready_bitmap", &self.task_ready_bitmap)
            .field("task_ready_queue", &self.task_ready_queue)
            .finish()
    }
}

impl<
        System: KernelCfg2,
        PortTaskState: 'static,
        TaskReadyBitmap: PrioBitmap,
        TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<System, PortTaskState, TaskPriority>>]> + Init + 'static,
        TaskPriority,
    > State<System, PortTaskState, TaskReadyBitmap, TaskReadyQueue, TaskPriority>
{
    /// Get the currently running task.
    pub fn running_task(&self) -> Option<&'static TaskCb<System, PortTaskState, TaskPriority>> {
        self.running_task.load(Ordering::Relaxed)
    }

    /// Get a reference to the variable storing the currently running task.
    ///
    /// # Safety
    ///
    /// Modifying the stored value is not allowed.
    pub unsafe fn active_task_ref(
        &self,
    ) -> &AtomicRef<'static, TaskCb<System, PortTaskState, TaskPriority>> {
        &self.running_task
    }
}
