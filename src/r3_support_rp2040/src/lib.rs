//! Supporting package for running [R3] on RP2040, which is used by
//! [Raspberry Pi Pico].
//!
//! [R3]: ::r3
//! [Raspberry Pi Pico]: https://pico.raspberrypi.org
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![feature(raw_ref_op)]
#![feature(maybe_uninit_ref)]
#![feature(asm)]
#![feature(const_fn)]
#![feature(const_mut_refs)]
#![feature(const_fn_fn_ptr_basics)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]

pub mod clock;
pub mod serial;
pub mod stdout;
mod usb;
pub mod usbstdio;
