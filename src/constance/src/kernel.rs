//! The RTOS kernel
use atomic_ref::AtomicRef;
use core::{mem::forget, num::NonZeroUsize, sync::atomic::Ordering};

use crate::utils::{Init, PrioBitmap};

#[macro_use]
mod cfg;
mod error;
mod hunk;
mod task;
mod utils;
pub use self::{cfg::*, error::*, hunk::*, task::*};

/// Numeric value used to identify various kinds of kernel objects.
pub type Id = NonZeroUsize;

/// Represents "system" types having sufficient trait `impl`s to instantiate the
/// kernel.
pub trait Kernel: Port + KernelCfg + Sized + 'static {}
impl<T: Port + KernelCfg + 'static> Kernel for T {}

/// Implemented by a port.
///
/// # Safety
///
/// Implementing a port is inherently unsafe because it's responsible for
/// initializing the execution environment and providing a dispatcher
/// implementation.
///
/// These methods are only meant to be called by the kernel.
pub unsafe trait Port: Sized {
    type PortTaskState: Send + Sync + Init + 'static;

    /// The initial value of [`TaskCb::port_task_state`] for all tasks.
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
    unsafe fn initialize_task_state(task: &'static task::TaskCb<Self, Self::PortTaskState>);

    /// Return a flag indicating whether a CPU Lock state is active.
    fn is_cpu_lock_active() -> bool;
}

/// Methods intended to be called by a port.
///
/// # Safety
///
/// These are only meant to be called by the port.
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

        // TODO: Choose only an active task
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
pub unsafe trait KernelCfg: Port + Sized {
    #[doc(hidden)]
    const HUNK_ATTR: HunkAttr;

    type TaskReadyBitmap: PrioBitmap;

    /// Access the kernel's global state.
    fn state() -> &'static State<Self, Self::PortTaskState, Self::TaskReadyBitmap>;

    // FIXME: Waiting for <https://github.com/rust-lang/const-eval/issues/11>
    //        to be resolved because `TaskCb` includes interior mutability
    //        and can't be referred to by `const`
    #[doc(hidden)]
    fn task_cb_pool() -> &'static [TaskCb<Self, Self::PortTaskState>];

    #[doc(hidden)]
    #[inline(always)]
    fn get_task_cb(i: usize) -> Option<&'static TaskCb<Self, Self::PortTaskState>> {
        Self::task_cb_pool().get(i)
    }
}

/// Global kernel state.
pub struct State<System: 'static, PortTaskState: 'static, TaskReadyBitmap: PrioBitmap> {
    // TODO: Make `running_task` non-null to simplify runtime code
    /// The currently running task.
    running_task: AtomicRef<'static, TaskCb<System, PortTaskState>>,

    /// The task ready bitmap, in which each bit indicates whether the
    /// task ready queue corresponding to that bit contains a task or not.
    task_ready_bitmap: utils::CpuLockCell<System, TaskReadyBitmap>,
}

impl<System: 'static, PortTaskState: 'static, TaskReadyBitmap: PrioBitmap> Init
    for State<System, PortTaskState, TaskReadyBitmap>
{
    const INIT: Self = Self {
        running_task: AtomicRef::new(None),
        task_ready_bitmap: Init::INIT,
    };
}

impl<System: 'static, PortTaskState: 'static, TaskReadyBitmap: PrioBitmap>
    State<System, PortTaskState, TaskReadyBitmap>
{
    /// Get the currently running task.
    pub fn running_task(&self) -> Option<&'static TaskCb<System, PortTaskState>> {
        self.running_task.load(Ordering::Relaxed)
    }

    /// Get a reference to the variable storing the currently running task.
    ///
    /// # Safety
    ///
    /// Modifying the stored value is not allowed.
    pub unsafe fn active_task_ref(&self) -> &AtomicRef<'static, TaskCb<System, PortTaskState>> {
        &self.running_task
    }
}
