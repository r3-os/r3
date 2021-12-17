#![feature(const_fn_trait_bound)]
#![feature(const_ptr_offset)]
#![feature(const_mut_refs)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_ptr_offset_from)]
#![feature(const_refs_to_cell)]
#![feature(const_trait_impl)]
#![feature(decl_macro)]
#![feature(asm_const)]
#![feature(asm_sym)]
#![feature(asm)]
#![feature(naked_functions)]
#![feature(slice_ptr_len)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unsupported_naked_functions)]
#![allow(clippy::verbose_bit_mask)] // questionable
#![doc(html_logo_url = "https://r3-os.github.io/r3/logo-small.svg")]
#![doc = include_str!("./lib.md")]
#![no_std]

/// Used by `use_port!`
#[doc(hidden)]
pub extern crate r3;

/// Used by `use_port!`
#[doc(hidden)]
pub extern crate r3_kernel;

/// Used by `use_sp804!`
#[doc(hidden)]
pub extern crate r3_portkit;

/// Used by `use_port!`
#[doc(hidden)]
#[cfg(target_os = "none")]
pub extern crate core;

#[cfg(doc)]
#[doc = include_str!("../CHANGELOG.md")]
pub mod _changelog_ {}

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
