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

/// The Platform-Level Interrupt Controller driver.
#[doc(hidden)]
pub mod plic {
    pub mod cfg;
    pub mod imp;
}

/// The binding for [`::riscv_rt`].
#[doc(hidden)]
pub mod rt {
    pub mod cfg;
}

/// The [`constance::kernel::PortThreading`] implementation.
#[doc(hidden)]
pub mod threading {
    pub mod cfg;
    pub mod imp;
}

pub use self::plic::cfg::*;
pub use self::rt::cfg::*;
pub use self::threading::cfg::*;

/// Defines the entry points of a port instantiation. Implemented by
/// [`use_port!`].
pub trait EntryPoint {
    /// Proceed with the boot process.
    ///
    /// # Safety
    ///
    ///  - The processor should be in M-mode and have M-mode interrupts masked.
    ///  - This method hasn't been entered yet.
    ///
    unsafe fn start() -> !;
}

/// An abstract interface to an interrupt controller. Implemented by
/// [`use_plic!`].
pub trait InterruptController {
    /// Initialize the driver. This will be called just before entering
    /// [`PortToKernel::boot`].
    ///
    /// [`PortToKernel::boot`]: constance::kernel::PortToKernel::boot
    ///
    /// # Safety
    ///
    /// This is only intended to be called by the port.
    unsafe fn init() {}

    /// Get the currently signaled interrupt and acknowledge it.
    ///
    /// # Safety
    ///
    /// This is only intended to be called by the port in an interrupt handler.
    unsafe fn acknowledge_interrupt() -> Option<constance::kernel::InterruptNum>;

    /// Notify that the kernel has completed the processing of the specified
    /// interrupt.
    ///
    /// # Safety
    ///
    /// This is only intended to be called by the port in an interrupt handler.
    unsafe fn end_interrupt(num: constance::kernel::InterruptNum);
}
