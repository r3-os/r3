#![feature(const_fn_trait_bound)]
#![feature(const_mut_refs)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(asm)]
#![feature(decl_macro)]
#![feature(generic_const_exprs)]
#![feature(const_maybe_uninit_as_ptr)]
#![feature(const_ptr_offset_from)]
#![feature(const_raw_ptr_deref)]
#![feature(const_refs_to_cell)]
#![feature(slice_ptr_len)]
#![feature(naked_functions)]
#![feature(const_trait_impl)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unsupported_naked_functions)]
#![doc = include_str!("./lib.md")]
#![no_std]

#[cfg(doc)]
#[doc = include_str!("../CHANGELOG.md")]
pub mod _changelog_ {}

/// The [`r3_kernel::PortThreading`] implementation.
#[doc(hidden)]
pub mod threading {
    pub mod cfg;
    #[cfg(target_os = "none")]
    pub mod imp;
}

/// The binding for [`::cortex_m_rt`].
#[doc(hidden)]
pub mod rt {
    pub mod cfg;
    #[cfg(target_os = "none")]
    pub mod imp;
}

/// The tickful [`r3_kernel::PortTimer`] implementation based on SysTick.
#[doc(hidden)]
pub mod systick_tickful {
    pub mod cfg;
    #[cfg(target_os = "none")]
    pub mod imp;
}

pub use self::{systick_tickful::cfg::*, threading::cfg::*};

/// Used by `use_port!`
#[doc(hidden)]
#[cfg(target_os = "none")]
pub extern crate core;
/// Used by `use_port!`
#[doc(hidden)]
pub extern crate r3;
/// Used by `use_port!`
#[doc(hidden)]
pub extern crate r3_kernel;
/// Used by `use_port!`
#[doc(hidden)]
#[cfg(target_os = "none")]
pub use cortex_m_rt;
