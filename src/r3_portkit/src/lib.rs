//! R3 PortKit
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_fn_trait_bound)]
#![feature(generic_const_exprs)]
#![feature(adt_const_params)]
#![feature(core_panic)]
#![feature(decl_macro)]
#![cfg_attr(
    feature = "doc",
    doc(html_logo_url = "https://r3-os.github.io/r3/logo-small.svg")
)]
#![no_std]

#[cfg(doc)]
#[doc = include_str!("../CHANGELOG.md")]
pub mod _changelog_ {}

#[macro_use]
pub mod utils;

pub mod num;
pub mod pptext;
pub mod sym;
pub mod tickful;
pub mod tickless;
