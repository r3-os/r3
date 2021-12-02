//! The kernel interface.
//!
//! Not to be confused with [`r3_kernel`][], a kernel implementation.
//!
//! [`r3_kernel`]: https://crates.io/crates/r3_kernel

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

/// The prelude module. This module re-exports [`Kernel`][2] with
/// impl-only-use (`use ... as _`, [RFC2166][1]).
///
/// <div class="admonition-follows"></div>
///
/// > **Rationale:** A prelude module is usually imported with a wildcard
/// > import (`use ...::prelude::*`). Name collisions caused by a wildcard
/// > import are difficult to notice (but cause a very confusing error) and
/// > fragile against otherwise-harmless upstream changes because imported
/// > names are not explicitly spelled in the source code.
/// >
/// > `Kernel` is not designed to be used in trait bounds, and system types are
/// > not supposed to have an associated function conflicting with those from
/// > `Kernel`. For these reasons, it's mostly useless to import the name
/// > `Kernel`.
///
/// [1]: https://rust-lang.github.io/rfcs/2166-impl-only-use.html
/// [2]: crate::kernel::Kernel
#[doc = include_str!("../common.md")]
pub mod prelude {
    #[doc(no_inline)]
    pub use super::Kernel as _;
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
