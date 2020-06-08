#![feature(external_doc)] // `#[doc(include = ...)]`
#![feature(const_fn)]
#![feature(const_if_match)]
#![feature(const_panic)]
#![feature(const_loop)]
#![feature(const_generics)]
#![feature(const_slice_from_raw_parts)]
#![feature(const_raw_ptr_deref)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![doc(include = "./lib.md")]
#![cfg_attr(not(test), no_std)] // Link `std` only when building a test (`cfg(test)`)

// When using `#![no_std]`, `core` has to be imported manually to be used
#[cfg(test)]
extern crate core;

#[macro_use]
pub mod kernel;
pub mod sync;
pub mod utils;

/// The prelude module.
pub mod prelude {
    #[doc(no_inline)]
    pub use crate::{kernel::Kernel, utils::Init};
}
