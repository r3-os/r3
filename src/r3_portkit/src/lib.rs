//! R3 PortKit
#![feature(generic_const_exprs)]
#![feature(adt_const_params)]
#![feature(naked_functions)]
#![feature(core_panic)]
#![feature(decl_macro)]
#![feature(asm_const)]
#![feature(const_cmp)]
#![feature(asm_sym)]
#![cfg_attr(
    feature = "doc",
    doc(html_logo_url = "https://r3-os.github.io/r3/logo-small.svg")
)]
#![no_std]

extern crate r3_core_ks as r3_core;

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
