//! Supporting package for running [R3] on a Renesas RZ/A1x family MPU
//! (including [RZ/A1H], which is used by the [GR-PEACH] development board).
//!
//! [R3]: ::r3
//! [RZ/A1H]: https://www.renesas.com/us/en/products/microcontrollers-microprocessors/rz/rza/rza1h.html
//! [GR-PEACH]: https://www.renesas.com/us/en/products/gadget-renesas/boards/gr-peach.html
#![feature(const_mut_refs)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_fn_trait_bound)]
#![feature(const_trait_impl)]
#![feature(asm)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unsupported_naked_functions)]
#![no_std]

/// Used by `use_os_timer!`
#[doc(hidden)]
pub extern crate r3;

/// Used by `use_os_timer!`
#[doc(hidden)]
pub extern crate r3_kernel;

/// Used by `use_os_timer!`
#[doc(hidden)]
pub extern crate r3_portkit;

/// Used by `use_os_timer!`
#[doc(hidden)]
pub extern crate r3_port_arm;

#[cfg(doc)]
#[doc = include_str!("../CHANGELOG.md")]
pub mod _changelog_ {}

/// The RZ/A1 OS Timer driver.
#[doc(hidden)]
pub mod os_timer {
    pub mod cfg;
    pub mod imp;
}

pub use self::os_timer::cfg::*;

pub mod gpio;
pub mod serial;
pub mod stdout;
