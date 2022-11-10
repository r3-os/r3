#![feature(const_maybe_uninit_array_assume_init)]
#![feature(const_fn_floating_point_arithmetic)]
#![feature(const_nonnull_slice_from_raw_parts)]
#![feature(const_maybe_uninit_uninit_array)]
#![feature(const_maybe_uninit_assume_init)]
#![feature(const_slice_from_raw_parts_mut)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(const_maybe_uninit_as_mut_ptr)]
#![feature(nonnull_slice_from_raw_parts)]
#![feature(type_changing_struct_update)]
#![feature(maybe_uninit_uninit_array)]
#![feature(const_precise_live_drops)]
#![feature(const_raw_ptr_comparison)]
#![feature(generic_associated_types)]
#![feature(associated_type_bounds)]
#![feature(const_slice_first_last)]
#![feature(cfg_target_has_atomic)] // `#[cfg(target_has_atomic_load_store)]`
#![feature(const_cell_into_inner)]
#![feature(type_alias_impl_trait)]
#![feature(const_slice_ptr_len)]
#![feature(exhaustive_patterns)] // `let Ok(()) = Ok::<(), !>(())`
#![feature(generic_const_exprs)]
#![feature(const_refs_to_cell)]
#![feature(maybe_uninit_slice)]
#![feature(const_nonnull_new)]
#![feature(const_result_drop)]
#![feature(const_slice_index)]
#![feature(unboxed_closures)] // `impl FnOnce`
#![feature(const_option_ext)]
#![feature(const_trait_impl)]
#![feature(const_ptr_write)]
#![feature(core_intrinsics)]
#![feature(assert_matches)]
#![feature(const_mut_refs)]
#![feature(const_ptr_read)]
#![feature(specialization)]
#![feature(const_convert)]
#![feature(const_type_id)] // `TypeId::of` as `const fn`
#![feature(set_ptr_value)] // `<*const T>::set_ptr_value`
#![feature(slice_ptr_len)]
#![feature(const_option)]
#![feature(cell_update)]
#![feature(const_deref)]
#![feature(const_heap)]
#![feature(const_swap)]
#![feature(decl_macro)]
#![feature(never_type)] // `!`
#![feature(const_try)]
#![feature(fn_traits)] // `impl FnOnce`
#![feature(let_else)]
#![feature(doc_cfg)] // `#[doc(cfg(...))]`
#![cfg_attr(test, feature(is_sorted))]
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(
    feature = "doc",
    doc(html_logo_url = "https://r3-os.github.io/r3/logo-small.svg")
)]
#![doc = include_str!("./lib.md")]
#![doc = include_str!("./common.md")]
#![doc = include!("../doc/system_lifecycle.rs")] // `![system_lifecycle]`
#![doc = include!("../doc/trait_binding.rs")] // `![trait_binding]`
#![doc = include!("../doc/static_cfg.rs")] // `![static_cfg]`
#![cfg_attr(
    feature = "_full",
    doc = r#"<style type="text/css">.disabled-feature-warning { display: none; }</style>"#
)]
#![cfg_attr(not(test), no_std)] // Link `std` only when building a test (`cfg(test)`)

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
pub mod bind;
pub mod closure;
pub mod hunk;
pub mod time;

/// The prelude module.
pub mod prelude {
    #[doc(no_inline)]
    pub use crate::bind::{ExecutableDefinerExt, UnzipBind};
    #[doc(no_inline)]
    pub use crate::kernel::prelude::*;
    #[doc(no_inline)]
    pub use crate::utils::Init;
}
