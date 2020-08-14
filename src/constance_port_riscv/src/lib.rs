#![feature(external_doc)]
#![feature(const_fn)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![doc(include = "./lib.md")]
#![no_std]

/// Used by macros
#[doc(hidden)]
pub extern crate constance;

/// Used by macros
#[doc(hidden)]
pub extern crate core;

/// The [`constance::kernel::PortThreading`] implementation.
#[doc(hidden)]
pub mod threading {
    pub mod cfg;
    pub mod imp;
}

pub use self::threading::cfg::*;

/// Defines the entry points of a port instantiation. Implemented by
/// [`use_port!`].
pub trait EntryPoint {
    /// Proceed with the boot process.
    ///
    /// # Safety
    ///
    ///  - The processor should be in M-mode and with M-mode interrupts masked.
    ///  - This method hasn't been entered yet.
    ///
    unsafe fn start() -> !;
}
