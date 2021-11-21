//! The kernel interface.

pub mod cfg;
mod error;
pub mod event_group;
pub mod hook;
pub mod hunk;
pub mod interrupt;
mod kernel;
pub mod mutex;
pub mod raw;
pub mod raw_cfg;
pub mod semaphore;
pub mod task;
pub mod timer;
pub use {
    cfg::Cfg,
    error::*,
    event_group::{EventGroup, EventGroupBits, EventGroupWaitFlags},
    hook::StartupHook,
    hunk::Hunk,
    interrupt::{InterruptHandler, InterruptLine, InterruptNum, InterruptPriority},
    kernel::*,
    mutex::{Mutex, MutexProtocol},
    raw::{Id, QueueOrder},
    semaphore::{Semaphore, SemaphoreValue},
    task::Task,
    timer::Timer,
};

/// The prelude module. This module re-exports [`Kernel`].
pub mod prelude {
    #[doc(no_inline)]
    pub use super::Kernel;
}

/// Re-exports all traits defined under this module for convenience.
pub mod traits {
    #[doc(no_inline)]
    pub use super::{
        cfg::KernelStatic,
        raw::{
            KernelAdjustTime, KernelBase, KernelBoostPriority, KernelEventGroup,
            KernelInterruptLine, KernelMutex, KernelSemaphore, KernelTaskSetPriority, KernelTime,
            KernelTimer,
        },
        raw_cfg::{
            CfgBase, CfgEventGroup, CfgInterruptLine, CfgMutex, CfgSemaphore, CfgTask, CfgTimer,
        },
        Kernel,
    };
}
