#![feature(const_fn_trait_bound)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_fn_floating_point_arithmetic)]
#![feature(generic_const_exprs)]
#![feature(const_ptr_offset)]
#![feature(const_mut_refs)]
#![feature(const_slice_from_raw_parts)]
#![feature(const_raw_ptr_deref)]
#![feature(const_option)]
#![feature(exhaustive_patterns)] // `let Ok(()) = Ok::<(), !>(())`
#![feature(decl_macro)]
#![feature(set_ptr_value)] // `<*const T>::set_ptr_value`
#![feature(option_result_unwrap_unchecked)] // `Option<T>::unwrap_unchecked`
#![feature(cfg_target_has_atomic)] // `#[cfg(target_has_atomic_load_store)]`
#![feature(never_type)] // `!`
#![feature(doc_cfg)] // `#[doc(cfg(...))]`
#![feature(specialization)]
#![feature(cell_update)]
#![feature(arbitrary_enum_discriminant)]
#![feature(untagged_unions)] // `union` with non-`Copy` fields
#![cfg_attr(test, feature(is_sorted))]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unsupported_naked_functions)]
#![doc = include_str!("./lib.md")]
#![doc = include_str!("./common.md")]
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

// `build!` requires `ArrayVec`
#[doc(hidden)]
pub extern crate arrayvec;

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
