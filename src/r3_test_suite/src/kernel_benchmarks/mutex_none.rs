//! Measures the execution times of mutex operations using a mutex created with
//! [`None`] as its locking protocol.
//!
//! [`None`]: r3::kernel::MutexProtocol::None
//!
//! ```text
//!             ┌─────┐                  ┌────┐
//!      mtx   pri  main               task1 pri
//!      │1│   │3│   │ │                 ┊    ┊
//!      │ │   │ │   │ │                 ┊    ┊     ┐
//!      ├─┤   ├─┤   │ │ mtx lock        ┊    ┊     │ I_LOCK
//!      │0│   │1│   │ │                 ┊    ┊     ┘
//!      │ │   │ │   │ │    activate     ┊    ┊
//!      │ │   │ │   │ │ ─────────────►  ┊    ┊
//!      │ │   │ │   │ │       park      ┊    ┊    
//!      │ │   │ │   └┬┘ ─────────────► ┌┴┐  ┌┴┐
//!      │ │   │ │    │                 │ │  │1│
//!      │ │   │ │    │     mtx lock    │ │  │ │
//!      │ │   │ │   ┌┴┐ ◀───────────── └┬┘  │ │
//!      │ │   │ │   │ │                 │   │ │
//!      │ │   │ │   │ │   mtx unlock    │   │ │    ┐
//!      ├─┤   ├─┤   └┬┘ ─────────────► ┌┴┐  │ │    │ I_UNLOCK_DISPATCING
//!      │0│   │3│    ┊                 │ │  │ │    ┘
//!      │ │   │ │    ┊                 │ │  │ │             ┐
//!      ├─┤   │ │    ┊                 │ │  │ │ mtx unlock  │ I_UNLOCK
//!      │1│   │ │    ┊    exit_task    │ │  │ │             ┘
//!      │ │   │ │   ┌┴┐ ◀───────────── └┬┘  └┬┘
//!      │ │   │ │   │ │                 ┊    ┊
//!
//!  pri: effective priority (assuming mtx uses the priority ceiling protocol)
//! ```
//!
use r3::kernel::traits;

pub use super::mutex::SupportedSystem;

use_benchmark_in_kernel_benchmark! {
    #[cfg_bounds(~const traits::CfgMutex)]
    pub unsafe struct App<System: SupportedSystem> {
        inner: super::mutex::AppInnerNone<System>,
    }
}
