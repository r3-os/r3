//! Measures the execution times of mutex operations using a mutex created with
//! [`Ceiling`] as its locking protocol.
//!
//! See [`mutex_none`] for the sequence diagram of this test.
//!
//! [`Ceiling`]: r3::kernel::MutexProtocol::Ceiling
//! [`mutex_none`]: crate::kernel_benchmarks::mutex_none
use r3::kernel::traits;

pub use super::mutex::SupportedSystem;

use_benchmark_in_kernel_benchmark! {
    #[cfg_bounds(~const traits::CfgMutex)]
    pub unsafe struct App<System: SupportedSystem> {
        inner: super::mutex::AppInnerCeiling<System>,
    }
}
