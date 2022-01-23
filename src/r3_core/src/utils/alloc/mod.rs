//! Compile-time memory allocation
#[macro_use]
mod vec;
mod allocator;
mod freeze;
mod rlsf;
pub use {allocator::*, freeze::*, vec::*};
