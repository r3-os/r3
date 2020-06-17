#![feature(external_doc)] // `#[doc(include = ...)]`
#![feature(const_fn)]
#![feature(const_if_match)]
#![feature(const_panic)]
#![feature(const_loop)]
#![feature(const_generics)]
#![feature(const_mut_refs)]
#![feature(const_slice_from_raw_parts)]
#![feature(const_raw_ptr_deref)]
#![feature(ptr_wrapping_offset_from)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![feature(never_type)] // `!`
#![cfg_attr(test, feature(is_sorted))]
#![deny(unsafe_op_in_unsafe_fn)]
#![doc(include = "./lib.md")]
#![cfg_attr(not(test), no_std)] // Link `std` only when building a test (`cfg(test)`)

// When using `#![no_std]`, `core` has to be imported manually to be used
#[cfg(test)]
extern crate core;

// `configure!` requires macros from this crate
#[doc(hidden)]
pub extern crate parse_generics_shim;

#[macro_use]
pub mod utils;
#[macro_use]
pub mod kernel;
pub mod sync;

/// The prelude module.
pub mod prelude {
    #[doc(no_inline)]
    pub use crate::{kernel::Kernel, utils::Init};
}
