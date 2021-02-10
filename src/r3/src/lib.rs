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
#![feature(exhaustive_patterns)] // `let Ok(()) = Ok::<(), !>(())`
#![feature(decl_macro)]
#![feature(set_ptr_value)] // `<*const T>::set_ptr_value`
#![feature(raw_ref_macros)]
#![feature(or_patterns)]
#![feature(option_result_unwrap_unchecked)] // `Option<T>::unwrap_unchecked`
#![feature(cfg_target_has_atomic)] // `#[cfg(target_has_atomic_load_store)]`
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![feature(never_type)] // `!`
#![feature(doc_cfg)] // `#[doc(cfg(...))]`
#![feature(specialization)]
#![feature(int_bits_const)]
#![feature(cell_update)]
#![feature(arbitrary_enum_discriminant)]
#![feature(untagged_unions)] // `union` with non-`Copy` fields
#![cfg_attr(test, feature(is_sorted))]
#![deny(unsafe_op_in_unsafe_fn)]
#![doc(include = "./lib.md")]
#![doc(include = "./common.md")]
#![cfg_attr(
    feature = "_full",
    doc = r#"<style type="text/css">.disabled-feature-warning { display: none; }</style>"#
)]
#![cfg_attr(not(test), no_std)] // Link `std` only when building a test (`cfg(test)`)

//   ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡀⣠⢊⠆⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
//   ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣤⣮⣵⡼⠁⡸⠉⠑⠢⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
//   ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡰⠢⢄⣀⠤⠒⠉⡉⠉⡉⠀⢱⣿⣦⠀⠀⠈⢣⡀⠀⠀⠀⠀⢀⣀⢠⣄⣴⣄⣤⣀⣀⠀⠀⠀⠀⠀⠀
//   ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡇⠀⠀⠐⠒⢄⠀⠲⣄⠄⣀⡨⠿⣛⠃⠉⠉⠀⠁⠀⢀⣄⣷⣾⡿⠿⠻⣏⡽⠿⠿⣿⣶⣇⣄⠀⠀⠀
//   ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢣⠈⠄⠀⠀⠀⢇⡀⠇⢉⠄⠈⠁⠁⣃⠀⠀⠀⠀⢰⣶⣿⣿⣥⣤⣤⣤⣤⣤⣤⣄⣀⠙⢻⣿⣶⠀⠀
//   ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠠⡣⡀⠀⠀⠀⠈⣀⠔⢁⣠⡀⠀⠣⢿⠁⠀⠀⣈⣿⠟⣿⠿⣿⣿⣿⠿⠻⠻⢿⣿⣿⣷⢠⡟⢿⣟⡀
//   ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡇⢣⠀⠀⠀⢊⡠⠊⠁⠾⠗⠀⡀⢈⡱⠀⠀⣽⣿⡟⠉⢐⣿⣿⣿⣷⣶⣾⣿⣿⣿⠁⠈⠙⣿⣿⣅
//   ⠀⠀⠀⠀⠀⠀⢀⡠⠔⠐⠒⠒⠒⠂⠤⣀⠀⠀⠀⠀⠀⠀⠀⢸⢸⠀⠀⠀⠀⠃⠒⠀⠀⠀⠀⢭⠁⠀⠀⠀⢼⣿⣧⣄⣔⣿⣿⣿⣇⣡⠉⢿⣿⣿⣧⣠⣿⣿⣿⠆
//   ⠀⠀⠀⠀⢀⠔⠁⢠⣶⣿⣿⣿⣿⣷⣶⣤⡑⢄⠀⠀⠀⠀⠀⠀⢎⡆⠀⠀⠀⠀⡪⠂⢚⠈⠉⠀⠀⠀⠀⠀⠐⢿⣿⣿⡿⢿⠿⠿⠿⠿⠀⠘⠿⠿⠿⢿⣿⣿⠗⠀
//   ⠀⠀⠀⡠⠃⠀⣰⣿⣿⣿⠟⠋⠉⠉⣉⣙⠻⢷⡩⠒⠦⢗⠖⢤⡈⡇⠀⠀⠀⠀⢸⠪⠥⡄⠀⠀⠀⠀⠀⠀⠀⠈⢛⣿⣿⣙⣧⣀⠀⠀⠀⢀⣀⣿⣹⣿⣿⠉⠁⠀
//   ⠀⠀⡰⠁⠀⢀⣿⣿⣿⠏⠀⢀⠖⠉⠀⠀⠉⡸⢣⠄⠂⠰⡃⠀⢌⠕⠀⠀⠀⠀⠈⣿⣿⣇⣀⣀⡀⠀⠀⠀⠀⠀⠀⠀⠛⠻⡿⢿⣿⣿⡿⣿⡿⡿⠛⠃⠀⠀⠀⠀
//   ⠀⢰⠁⠠⠀⢸⣿⣿⣟⠀⢠⠋⠀⠀⠀⠀⢠⠣⠏⠱⢅⢘⠌⠀⢸⠀⠀⠀⠀⠀⢀⠞⠚⠉⠀⠀⠈⡆⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠀⠈⠀⠈⠀⠀⠀⠀⠀⠀⠀⠀
//   ⠀⡇⢠⠀⠀⢸⣿⣿⣗⠀⡎⠀⠀⠀⠀⠀⢸⠀⠁⠀⡌⠑⠢⠢⠃⠀⠀⠀⠀⡠⠃⠀⠀⠀⡔⠀⡰⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
//   ⢸⠀⣽⢀⠀⢐⣿⣿⣿⡀⡇⠀⠀⠀⠀⠀⠈⡆⠀⢀⠜⢒⠤⣀⣠⠁⠀⠠⠇⡇⠀⠀⠀⡠⢃⠔⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
//   ⠘⢄⡺⡄⠀⠀⣿⣿⣿⣇⠇⠀⠀⠀⠀⡔⠉⠀⢀⠎⠀⡜⠀⢀⠆⠀⠀⢸⠀⠉⠂⠃⠉⠉⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
//   ⠀⠀⠁⠣⢄⡀⢹⣿⣿⣿⣵⠀⠀⠀⡰⠁⠀⠀⡎⠀⡸⠀⠀⢸⠀⠀⠀⢸⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
//   ⠀⠀⠀⠀⠀⠈⠚⠿⣿⣿⣿⡆⠀⢠⠃⠀⠀⢸⠀⠀⡇⠀⠀⡇⠀⠀⠀⢸⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
//   ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠉⠛⠿⡄⡜⠀⠀⠀⢸⠀⠀⢢⠀⠀⡇⠀⠀⠀⠐⡅⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
//   ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡀⡇⠀⠀⠀⠈⣆⣀⡨⠆⠬⡆⠀⠀⠀⠀⢣⡠⡀⣀⢀⢀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
//   ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠑⠈⠑⠦⠰⠤⠢⠎⠢⡘⢌⢊⠧⠤⠤⠤⠤⠬⠒⠌⠢⠑⠐⠁⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀

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
pub mod hunk;
pub mod sync;
pub mod time;

/// The prelude module.
pub mod prelude {
    #[doc(no_inline)]
    pub use crate::{kernel::Kernel, utils::Init};
}
