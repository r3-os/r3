#![doc = include_str!("./lib.md")]
#![doc = include_str!("./common.md")]
#![cfg_attr(
    feature = "_full",
    doc = r#"<style type="text/css">.disabled-feature-warning { display: none; }</style>"#
)]
#![cfg_attr(not(test), no_std)] // Link `std` only when building a test (`cfg(test)`)
pub mod kernel;
pub mod utils;
