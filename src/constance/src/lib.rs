#![feature(external_doc)] // `#[doc(include = ...)]`
#![feature(const_fn)]
#![feature(const_panic)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_fn_floating_point_arithmetic)]
#![feature(const_generics)]
#![feature(const_ptr_offset)]
#![feature(const_mut_refs)]
#![feature(const_fn_union)]
#![feature(const_slice_from_raw_parts)]
#![feature(const_raw_ptr_deref)]
#![feature(const_checked_int_methods)]
#![feature(const_option)]
#![feature(cfg_target_has_atomic)] // `#[cfg(target_has_atomic_load_store)]`
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![feature(never_type)] // `!`
#![feature(specialization)]
#![feature(int_bits_const)]
#![feature(untagged_unions)] // `union` with non-`Copy` fields
#![cfg_attr(test, feature(is_sorted))]
#![deny(unsafe_op_in_unsafe_fn)]
#![doc(include = "./lib.md")]
#![doc(include = "./common.md")]
#![cfg_attr(not(test), no_std)] // Link `std` only when building a test (`cfg(test)`)

// `array_item_from_fn!` requires `MaybeUninit`.
#[doc(hidden)]
pub extern crate core;

// `build!` requires `StaticVec`
#[doc(hidden)]
pub extern crate staticvec;

#[macro_use]
pub mod utils;
#[macro_use]
pub mod kernel;
pub mod sync;
pub mod time;

/// The prelude module.
pub mod prelude {
    #[doc(no_inline)]
    pub use crate::{kernel::Kernel, utils::Init};
}
