//! R3 PortKit
#![feature(adt_const_params)]
#![feature(generic_const_exprs)]
#![feature(const_fn_trait_bound)]
#![feature(core_panic)]
#![feature(decl_macro)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(asm)]
#![deny(unsupported_naked_functions)]
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
