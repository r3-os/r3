#![feature(arbitrary_enum_discriminant)]
#![feature(const_fn_trait_bound)]
#![feature(const_trait_impl)]
#![feature(const_mut_refs)]
#![feature(cell_update)]
#![feature(decl_macro)]
#![feature(doc_cfg)]
#![deny(unsafe_op_in_unsafe_fn)]
#![doc(html_logo_url = "https://r3-os.github.io/r3/logo-small.svg")]
#![doc = include_str!("./lib.md")]
#![doc = include_str!("./common.md")]
// FIXME: Work-around for <https://github.com/rust-lang/rust/issues/32104>
#![cfg_attr(feature = "doc",
    doc = embed_doc_image::embed_image!("R3 Real-Time Operating System", "doc/logo-large.svg"),
)]
#![cfg_attr(
    feature = "_full",
    doc = r#"<style type="text/css">.disabled-feature-warning { display: none; }</style>"#
)]
#![no_std]

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

#[cfg(doc)]
#[doc = include_str!("../CHANGELOG.md")]
pub mod _changelog_ {}

pub mod bind;

#[cfg(feature = "sync")]
#[doc(cfg(feature = "sync"))]
pub mod sync;
mod tests;

pub use r3_core::{bag, hunk, kernel, time};

/// Utilities. This module re-exports items from [`r3_core::utils`] that are
/// subject to the application-side API stability guarantee.
pub mod utils {
    pub use r3_core::utils::{Init, ZeroInit};
}

/// The prelude module.
pub mod prelude {
    #[doc(no_inline)]
    pub use r3_core::prelude::*;
}
