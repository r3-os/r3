//! Safe synchronization primitives.
pub mod mutex;
pub mod recursive_mutex;
#[doc(no_inline)]
pub use self::{mutex::StaticMutex, recursive_mutex::StaticRecursiveMutex};
