//! Constance PortKit
#![feature(const_generics)]
#![feature(const_fn)]
#![feature(const_panic)]
#![feature(const_saturating_int_methods)]
#![feature(core_panic)]
#![feature(decl_macro)]
#![no_std]

#[macro_use]
pub mod utils;

pub mod pptext;
pub mod tickful;
pub mod tickless;
pub mod num;
