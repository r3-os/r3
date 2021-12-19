#![feature(const_maybe_uninit_assume_init)]
#![feature(const_fn_trait_bound)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_fn_floating_point_arithmetic)]
#![feature(generic_const_exprs)]
#![feature(const_ptr_offset)]
#![feature(const_swap)]
#![feature(const_slice_first_last)]
#![feature(maybe_uninit_slice)]
#![feature(const_mut_refs)]
#![feature(const_slice_from_raw_parts)]
#![feature(const_option)]
#![feature(const_default_impls)]
#![feature(const_trait_impl)]
#![feature(const_refs_to_cell)]
#![feature(exhaustive_patterns)] // `let Ok(()) = Ok::<(), !>(())`
#![feature(decl_macro)]
#![feature(set_ptr_value)] // `<*const T>::set_ptr_value`
#![feature(cfg_target_has_atomic)] // `#[cfg(target_has_atomic_load_store)]`
#![feature(never_type)] // `!`
#![feature(const_type_id)] // `TypeId::of` as `const fn`
#![feature(doc_cfg)] // `#[doc(cfg(...))]`
#![feature(specialization)]
#![feature(cell_update)]
#![feature(assert_matches)]
#![feature(arbitrary_enum_discriminant)]
#![feature(untagged_unions)] // `union` with non-`Copy` fields
#![cfg_attr(test, feature(is_sorted))]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unsupported_naked_functions)]
#![doc(html_logo_url = "https://r3-os.github.io/r3/logo-small.svg")]
#![doc = include_str!("./lib.md")]
#![doc = include_str!("./common.md")]
// FIXME: Work-around for <https://github.com/rust-lang/rust/issues/32104>
#![cfg_attr(feature = "doc",
    doc = embed_doc_image::embed_image!("R3 Real-Time Operating System", "doc/logo-large.svg"),
)]
#![doc = include!("../doc/trait_binding.rs")] // `![trait_binding]`
#![doc = include!("../doc/static_cfg.rs")] // `![static_cfg]`
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

#[cfg(doc)]
#[doc = include_str!("../CHANGELOG.md")]
pub mod _changelog_ {}

#[macro_use]
pub mod utils;
#[macro_use]
pub mod kernel;
pub mod bag;
pub mod hunk;
pub mod sync;
pub mod time;

/// The prelude module.
pub mod prelude {
    #[doc(no_inline)]
    pub use crate::kernel::prelude::*;
    #[doc(no_inline)]
    pub use crate::utils::Init;
}
