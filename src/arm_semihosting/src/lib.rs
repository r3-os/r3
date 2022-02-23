//! Semihosting for ARM Cortex-A processors (forked from [`cortex-m-semihosting`])
//!
//! [`cortex-m-semihosting`]: https://github.com/rust-embedded/cortex-m-semihosting
//!
//! # What is semihosting?
//!
//! "Semihosting is a mechanism that enables code running on an ARM target to communicate and use
//! the Input/Output facilities on a host computer that is running a debugger." - ARM
//!
//! # Interface
//!
//! This crate provides implementations of
//! [`core::fmt::Write`](https://doc.rust-lang.org/core/fmt/trait.Write.html), so you can use it,
//! in conjunction with
//! [`core::format_args!`](https://doc.rust-lang.org/core/macro.format_args.html) or the [`write!` macro](https://doc.rust-lang.org/core/macro.write.html), for user-friendly construction and printing of formatted strings.
//!
//! Since semihosting operations are modeled as [system calls][sc], this crate exposes an untyped
//! `syscall!` interface just like the [`sc`] crate does.
//!
//! [sc]: https://en.wikipedia.org/wiki/System_call
//! [`sc`]: https://crates.io/crates/sc
//!
//! # Forewarning
//!
//! Semihosting operations are *very* slow. Like, each WRITE operation can take hundreds of
//! milliseconds.
//!
//! # Example
//!
//! The usage is exact the same as `cortex-m-semihosting`.
//!
//! # Optional features
//!
//! ## `no-semihosting`
//!
//! When this feature is enabled, the underlying system calls to `bkpt` are patched out.
//!
//! # Reference
//!
//! For documentation about the semihosting operations, check:
//!
//! 'Chapter 8 - Semihosting' of the ['ARM Compiler toolchain Version 5.0'][pdf]
//! manual.
//!
//! [pdf]: http://infocenter.arm.com/help/topic/com.arm.doc.dui0471e/DUI0471E_developing_for_arm_processors.pdf

#![deny(missing_docs)]
#![allow(clippy::missing_safety_doc)]
#![no_std]

#[macro_use]
mod macros;

pub mod debug;
#[doc(hidden)]
pub mod export;
pub mod hio;
pub mod nr;

#[cfg(all(thumb, not(feature = "inline-asm")))]
extern "C" {
    fn __syscall(nr: usize, arg: usize) -> usize;
}

/// Performs a semihosting operation, takes a pointer to an argument block
#[inline(always)]
pub unsafe fn syscall<T>(nr: usize, arg: &T) -> usize {
    syscall1(nr, arg as *const T as usize)
}

/// Performs a semihosting operation, takes one integer as an argument
#[inline(always)]
pub unsafe fn syscall1(_nr: usize, _arg: usize) -> usize {
    match () {
        #[cfg(all(thumb, arm, not(feature = "no-semihosting")))]
        () => {
            use core::arch::asm;
            let mut nr = _nr;
            asm!("svc 0xAB", inout("r0") nr, in("r1") _arg, out("lr") _);
            nr
        }

        #[cfg(all(thumb, arm, feature = "no-semihosting"))]
        () => 0,

        #[cfg(all(not(thumb), arm, not(feature = "no-semihosting")))]
        () => {
            use core::arch::asm;
            let mut nr = _nr;
            asm!("svc 0x123456", inout("r0") nr, in("r1") _arg, out("lr") _);
            nr
        }

        #[cfg(all(not(thumb), arm, feature = "no-semihosting"))]
        () => 0,

        #[cfg(not(arm))]
        () => unimplemented!(),
    }
}
