//! Compile-time memory allocation
#[macro_use]
mod vec;
pub use vec::*;
mod allocator;
mod rlsf;
pub use allocator::*;
