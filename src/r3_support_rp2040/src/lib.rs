//! Supporting package for running [R3] on RP2040, which is used by
//! [Raspberry Pi Pico].
//!
//! [R3]: ::r3
//! [Raspberry Pi Pico]: https://pico.raspberrypi.org
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_fn_trait_bound)]
#![feature(exhaustive_patterns)]
#![feature(const_trait_impl)]
#![feature(const_mut_refs)]
#![feature(raw_ref_op)]
#![doc(html_logo_url = "https://r3-os.github.io/r3/logo-small.svg")]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]

#[cfg(doc)]
#[doc = include_str!("../CHANGELOG.md")]
pub mod _changelog_ {}

pub mod clock;
pub mod serial;
pub mod stdout;
mod usb;
pub mod usbstdio;
