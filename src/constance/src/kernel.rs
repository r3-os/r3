//! The RTOS kernel
use core::num::NonZeroUsize;

use crate::utils::Init;

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
pub trait Kernel: Port + KernelCfg + Sized {}
impl<T: Port + KernelCfg> Kernel for T {}

/// Implemented by a port.
///
/// # Safety
///
/// Implementing a port is inherently unsafe because it's responsible for
/// initializing the execution environment and providing a dispatcher
/// implementation.
///
/// Here's a non-comprehensive list of things a port is required to do:
///
///  - Call [`init_hunks`] before dispatching the first task.
///  - TODO
///
pub unsafe trait Port {
    type PortTaskState: Copy + Send + Sync + Init + 'static;
    const PORT_TASK_STATE_INIT: Self::PortTaskState;

    /// Yield the processor.
    ///
    /// Precondition: CPU Lock inactive
    ///
    /// # Safety
    ///
    /// This is meant to be only called by the kernel.
    unsafe fn yield_cpu();

    /// Disable all kernel-managed interrupts (this state is called *CPU Lock*).
    ///
    /// Precondition: CPU Lock inactive
    ///
    /// # Safety
    ///
    /// This is meant to be only called by the kernel.
    unsafe fn enter_cpu_lock();

    /// Re-enable kernel-managed interrupts previously disabled by
    /// `enter_cpu_lock`, thus deactivating the CPU Lock state.
    ///
    /// Precondition: CPU Lock active
    ///
    /// # Safety
    ///
    /// This is meant to be only called by the kernel.
    unsafe fn leave_cpu_lock();

    /// Return a flag indicating whether a CPU Lock state is active.
    fn is_cpu_lock_active() -> bool;
}

/// Associates "system" types with kernel-private data. Use [`build!`] to
/// implement.
///
/// # Safety
///
/// This is only intended to be implemented by `build!`.
pub unsafe trait KernelCfg: Port {
    #[doc(hidden)]
    const HUNK_ATTR: HunkAttr;

    // FIXME: Waiting for <https://github.com/rust-lang/const-eval/issues/11>
    //        to be resolved because `TaskCb` includes interior mutability
    //        and can't be referred to by `const`
    #[doc(hidden)]
    fn task_cb_pool() -> &'static [TaskCb<Self::PortTaskState>];

    #[doc(hidden)]
    #[inline(always)]
    fn get_task_cb(i: usize) -> Option<&'static TaskCb<Self::PortTaskState>> {
        Self::task_cb_pool().get(i)
    }
}
