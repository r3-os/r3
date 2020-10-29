#![feature(external_doc)]
#![feature(const_fn)]
#![feature(const_generics)]
#![feature(const_panic)]
#![feature(const_ptr_offset)]
#![feature(const_mut_refs)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(decl_macro)]
#![feature(asm)]
#![feature(naked_functions)]
#![feature(slice_ptr_len)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(clippy::verbose_bit_mask)] // questionable
#![doc(include = "./lib.md")]
#![no_std]

/// Used by `use_port!`
#[doc(hidden)]
pub extern crate constance;

/// Used by `use_sp804!`
#[doc(hidden)]
pub extern crate constance_portkit;

/// Used by `use_port!`
#[doc(hidden)]
#[cfg(target_os = "none")]
pub extern crate core;

#[cfg(target_os = "none")]
mod arm;

/// The thread management implementation for the Arm port.
#[doc(hidden)]
pub mod threading {
    pub mod cfg;
    #[cfg(target_os = "none")]
    pub mod imp;
}

/// The Arm Generic Interrupt Controller driver.
#[doc(hidden)]
pub mod gic {
    pub mod cfg;
    mod gic_regs;
    pub mod imp;
}

/// The standard startup code.
#[doc(hidden)]
pub mod startup {
    pub mod cfg;
    #[cfg(target_os = "none")]
    pub mod imp;
}

/// The SP804 Dual Timer driver.
#[doc(hidden)]
pub mod sp804 {
    pub mod cfg;
    pub mod imp;
    mod sp804_regs;
}

pub use self::gic::cfg::*;
pub use self::sp804::cfg::*;
pub use self::startup::cfg::*;
pub use self::threading::cfg::*;
