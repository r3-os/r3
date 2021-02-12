//! Supporting package for running [R3] on RP2040, which is used by
//! [Raspberry Pi Pico].
//!
//! [R3]: ::r3
//! [Raspberry Pi Pico]: https://pico.raspberrypi.org
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
