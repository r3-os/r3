//! Measures the execution times of mutex operations using a mutex created with
//! [`Ceiling`] as its locking protocol.
//!
//! [`Ceiling`]: constance::kernel::MutexProtocol::Ceiling
//!
//! See [`mutex_none`](super::mutex_none) for the sequence diagram of this test.
//!
use_benchmark_in_kernel_benchmark! {
    pub unsafe struct App<System> {
        inner: super::mutex::AppInnerCeiling<System>,
    }
}
