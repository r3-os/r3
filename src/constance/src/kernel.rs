//! The RTOS kernel
use core::num::NonZeroUsize;

use crate::utils::Init;

#[macro_use]
mod cfg;
mod error;
mod task;
pub use self::{cfg::*, error::*, task::*};

/// Numeric value used to identify various kinds of kernel objects.
pub type Id = NonZeroUsize;

/// Represents "system" types having sufficient trait `impl`s to instantiate the
/// kernel.
pub trait Kernel: Port + KernelCfg + Sized {}
impl<T: Port + KernelCfg> Kernel for T {}

pub unsafe trait Port {
    type PortTaskState: Copy + Send + Sync + Init + 'static;
    const PORT_TASK_STATE_INIT: Self::PortTaskState;

    fn dispatch();
}
