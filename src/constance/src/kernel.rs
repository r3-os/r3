//! The RTOS kernel
use core::num::NonZeroUsize;

use crate::utils::Init;

#[macro_use]
mod cfg;
mod error;
mod hunk;
mod task;
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

    fn dispatch();
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

    #[doc(hidden)]
    const TASK_STATE: &'static [TaskState<Self::PortTaskState>];

    #[doc(hidden)]
    const TASK_ATTR: &'static [TaskAttr];
}
