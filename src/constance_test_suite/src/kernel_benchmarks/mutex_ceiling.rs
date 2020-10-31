//! Measures the execution times of mutex operations using a mutex created with
//! [`Ceiling`] as its locking protocol.
//!
//! See [`mutex_none`] for the sequence diagram of this test.
//!
//! [`Ceiling`]: constance::kernel::MutexProtocol::Ceiling
//! [`mutex_none`]: crate::kernel_benchmarks::mutex_none
use_benchmark_in_kernel_benchmark! {
    pub unsafe struct App<System> {
        inner: super::mutex::AppInnerCeiling<System>,
    }
}
