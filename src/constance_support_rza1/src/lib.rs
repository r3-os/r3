//! Supporting package for running [Constance] on a Renesas RZ/A1x family MPU
//! (including [RZ/A1H], which is used by the [GR-PEACH] development board).
//!
//! [Constance]: ::constance
//! [RZ/A1H]: https://www.renesas.com/us/en/products/microcontrollers-microprocessors/rz/rza/rza1h.html
//! [GR-PEACH]: https://www.renesas.com/us/en/products/gadget-renesas/boards/gr-peach.html
#![feature(const_fn)]
#![feature(const_mut_refs)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(asm)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]

/// Used by `use_os_timer!`
#[doc(hidden)]
pub extern crate constance;

/// Used by `use_os_timer!`
#[doc(hidden)]
pub extern crate constance_portkit;

/// Used by `use_os_timer!`
#[doc(hidden)]
pub extern crate constance_port_arm;

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
