//! The kernel interface.

pub mod cfg;
mod error;
pub mod event_group;
pub mod hook;
pub mod hunk;
pub mod interrupt;
pub mod mutex;
pub mod raw;
pub mod raw_cfg;
pub mod task;
pub use {
    cfg::Cfg,
    error::*,
    event_group::{EventGroup, EventGroupBits, EventGroupWaitFlags},
    hook::StartupHook,
    hunk::Hunk,
    interrupt::{InterruptLine, InterruptNum, InterruptPriority},
    mutex::{Mutex, MutexProtocol},
    raw::{Id, QueueOrder},
    task::Task,
};

/// The prelude module.
///
/// This module re-exports traits from [`raw`] that defines global functions
/// of a system type. Other traits for which object-safe wrappers are provided,
/// e.g., [`raw::KernelMutex`], are not re-exported here.
pub mod prelude {
    #[doc(no_inline)]
    pub use super::raw::{KernelAdjustTime, KernelBase, KernelBoostPriority, KernelTime};
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
    };
}
