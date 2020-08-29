#![feature(external_doc)]
#![feature(const_fn)]
#![feature(decl_macro)]
#![feature(naked_functions)]
#![feature(slice_ptr_len)]
#![feature(asm)]
#![feature(raw_ref_op)]
#![feature(const_generics)]
#![feature(const_option)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![doc(include = "./lib.md")]
#![doc(include = "../../constance/src/common.md")]
#![no_std]
use constance::kernel::{
    ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
    PendInterruptLineError, QueryInterruptLineError, SetInterruptLinePriorityError,
};
use core::ops::Range;

/// Used by macros
#[doc(hidden)]
pub extern crate constance;

/// Used by macros
#[doc(hidden)]
pub extern crate constance_portkit;

/// Used by macros
#[doc(hidden)]
pub extern crate core;

/// The Platform-Level Interrupt Controller driver.
#[doc(hidden)]
pub mod plic {
    pub mod cfg;
    pub mod imp;
    pub mod plic_regs;
}

/// The binding for [`::riscv_rt`].
#[doc(hidden)]
pub mod rt {
    pub mod cfg;
    #[cfg(target_os = "none")]
    pub mod imp;
}

/// The [`constance::kernel::PortThreading`] implementation.
#[doc(hidden)]
pub mod threading {
    pub mod cfg;
    #[cfg(target_os = "none")]
    pub mod imp;
}

/// The standard timer driver.
#[doc(hidden)]
pub mod timer {
    pub mod cfg;
    pub mod imp;
}

pub use self::plic::cfg::*;
pub use self::rt::cfg::*;
pub use self::threading::cfg::*;
pub use self::timer::cfg::*;

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

    /// The trap handler.
    ///
    /// # Safety
    ///
    ///  - The processor should be in M-mode and have M-mode interrupts masked.
    ///  - The register state of the background context should be preserved so
    ///    that the handler can restore it later.
    ///
    unsafe fn exception_handler() -> !;
}

/// An abstract inferface to a port timer driver. Implemented by
/// [`use_timer!`].
pub trait Timer {
    /// Initialize the driver. This will be called just before entering
    /// [`PortToKernel::boot`].
    ///
    /// [`PortToKernel::boot`]: constance::kernel::PortToKernel::boot
    ///
    /// # Safety
    ///
    /// This is only intended to be called by the port.
    unsafe fn init() {}
}

/// An abstract interface to an interrupt controller. Implemented by
/// [`use_plic!`].
///
/// # Safety
///
/// These methods are only intended to be called by the port.
pub trait InterruptController {
    /// Initialize the driver. This will be called just before entering
    /// [`PortToKernel::boot`].
    ///
    /// [`PortToKernel::boot`]: constance::kernel::PortToKernel::boot
    unsafe fn init() {}

    /// The range of interrupt priority values considered [managed].
    ///
    /// Defaults to `0..0` (empty) when unspecified.
    ///
    /// [managed]: constance#interrupt-handling-framework
    #[allow(clippy::reversed_empty_ranges)] // on purpose
    const MANAGED_INTERRUPT_PRIORITY_RANGE: Range<InterruptPriority> = 0..0;

    /// Handle the call to [`PortInterrupts::set_interrupt_line_priority`] for a
    /// platform interrupt line.
    ///
    /// The provided interrupt number must be greater than or equal to
    /// [`INTERRUPT_PLATFORM_START`].
    ///
    /// [`PortInterrupts::set_interrupt_line_priority`]: constance::kernel::PortInterrupts::set_interrupt_line_priority
    unsafe fn set_interrupt_line_priority(
        _line: InterruptNum,
        _priority: InterruptPriority,
    ) -> Result<(), SetInterruptLinePriorityError> {
        Err(SetInterruptLinePriorityError::BadParam)
    }

    /// Handle the call to [`PortInterrupts::enable_interrupt_line`] for a
    /// platform interrupt line.
    ///
    /// The provided interrupt number must be greater than or equal to
    /// [`INTERRUPT_PLATFORM_START`].
    ///
    /// [`PortInterrupts::enable_interrupt_line`]: constance::kernel::PortInterrupts::enable_interrupt_line
    unsafe fn enable_interrupt_line(_line: InterruptNum) -> Result<(), EnableInterruptLineError> {
        Err(EnableInterruptLineError::BadParam)
    }

    /// Handle the call to [`PortInterrupts::disable_interrupt_line`] for a
    /// platform interrupt line.
    ///
    /// The provided interrupt number must be greater than or equal to
    /// [`INTERRUPT_PLATFORM_START`].
    ///
    /// [`PortInterrupts::disable_interrupt_line`]: constance::kernel::PortInterrupts::disable_interrupt_line
    unsafe fn disable_interrupt_line(_line: InterruptNum) -> Result<(), EnableInterruptLineError> {
        Err(EnableInterruptLineError::BadParam)
    }

    /// Handle the call to [`PortInterrupts::pend_interrupt_line`] for a
    /// platform interrupt line.
    ///
    /// The provided interrupt number must be greater than or equal to
    /// [`INTERRUPT_PLATFORM_START`].
    ///
    /// [`PortInterrupts::pend_interrupt_line`]: constance::kernel::PortInterrupts::pend_interrupt_line
    unsafe fn pend_interrupt_line(_line: InterruptNum) -> Result<(), PendInterruptLineError> {
        Err(PendInterruptLineError::BadParam)
    }

    /// Handle the call to [`PortInterrupts::clear_interrupt_line`] for a
    /// platform interrupt line.
    ///
    /// The provided interrupt number must be greater than or equal to
    /// [`INTERRUPT_PLATFORM_START`].
    ///
    /// [`PortInterrupts::clear_interrupt_line`]: constance::kernel::PortInterrupts::clear_interrupt_line
    unsafe fn clear_interrupt_line(_line: InterruptNum) -> Result<(), ClearInterruptLineError> {
        Err(ClearInterruptLineError::BadParam)
    }

    /// Handle the call to [`PortInterrupts::is_interrupt_line_pending`] for a
    /// platform interrupt line.
    ///
    /// The provided interrupt number must be greater than or equal to
    /// [`INTERRUPT_PLATFORM_START`].
    ///
    /// [`PortInterrupts::is_interrupt_line_pending`]: constance::kernel::PortInterrupts::is_interrupt_line_pending
    unsafe fn is_interrupt_line_pending(
        _line: InterruptNum,
    ) -> Result<bool, QueryInterruptLineError> {
        Err(QueryInterruptLineError::BadParam)
    }
}
