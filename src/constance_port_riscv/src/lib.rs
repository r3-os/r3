#![feature(external_doc)]
#![feature(const_fn)]
#![feature(naked_functions)]
#![feature(slice_ptr_len)]
#![feature(llvm_asm)]
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
    #[cfg(target_os = "none")]
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

    /// The Machine external interrupt and timer handler.
    ///
    /// # Safety
    ///
    ///  - The processor should be in M-mode and have M-mode interrupts masked.
    ///  - The register state of the background context should be preserved so
    ///    that the handler can restore it later.
    ///
    unsafe fn external_interrupt_handler() -> !;
}

/// An abstract interface to an interrupt controller. Implemented by
/// [`use_plic!`].
pub trait InterruptController {
    type Token;

    /// Initialize the driver. This will be called just before entering
    /// [`PortToKernel::boot`].
    ///
    /// [`PortToKernel::boot`]: constance::kernel::PortToKernel::boot
    ///
    /// # Safety
    ///
    /// This is only intended to be called by the port.
    unsafe fn init() {}

    /// Get the currently signaled interrupt and claim it. Raise the interrupt
    /// priority threshold to at least mask the claimed interrupt.
    ///
    /// # Safety
    ///
    /// This is only intended to be called by the port in an interrupt handler.
    unsafe fn claim_interrupt() -> Option<(Self::Token, constance::kernel::InterruptNum)>;

    /// Notify that the kernel has completed the processing of the specified
    /// interrupt claim. Restore the interrupt priority threshold.
    ///
    /// # Safety
    ///
    /// This is only intended to be called by the port in an interrupt handler.
    unsafe fn end_interrupt(token: Self::Token);
}
