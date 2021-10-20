//! The public interface of the GIC driver.
use r3::kernel::InterruptNum;
use tock_registers::{
    fields::FieldValue,
    interfaces::{ReadWriteable, Readable},
};

use super::{gic_regs, imp::GicRegs};

/// Implement [`PortInterrupts`], [`InterruptController`], and [`Gic`] on
/// the given system type using the General Interrupt Controller (GIC) on the
/// target.
/// **Requires [`GicOptions`].**
///
/// [`PortInterrupts`]: r3::kernel::PortInterrupts
/// [`InterruptController`]: crate::InterruptController
///
/// # Safety
///
///  - The target must really include a GIC.
///  - `GicOptions` should be configured correctly and the memory-mapped
///    registers should be accessible.
///
#[macro_export]
macro_rules! use_gic {
    (unsafe impl PortInterrupts for $sys:ty) => {
        const _: () = {
            use $crate::{
                core::ops::Range,
                gic::imp,
                r3::kernel::{
                    ClearInterruptLineError, EnableInterruptLineError, InterruptNum,
                    InterruptPriority, PendInterruptLineError, PortInterrupts,
                    QueryInterruptLineError, SetInterruptLinePriorityError,
                },
                Gic, InterruptController,
            };

            unsafe impl Gic for $sys {
                #[inline(always)]
                fn gic_regs() -> imp::GicRegs {
                    unsafe { imp::GicRegs::from_system::<Self>() }
                }
            }

            unsafe impl PortInterrupts for $sys {
                const MANAGED_INTERRUPT_PRIORITY_RANGE: Range<InterruptPriority> = 0..255;

                #[inline]
                unsafe fn set_interrupt_line_priority(
                    line: InterruptNum,
                    priority: InterruptPriority,
                ) -> Result<(), SetInterruptLinePriorityError> {
                    imp::set_interrupt_line_priority::<Self>(line, priority)
                }

                #[inline]
                unsafe fn enable_interrupt_line(
                    line: InterruptNum,
                ) -> Result<(), EnableInterruptLineError> {
                    imp::enable_interrupt_line::<Self>(line)
                }

                #[inline]
                unsafe fn disable_interrupt_line(
                    line: InterruptNum,
                ) -> Result<(), EnableInterruptLineError> {
                    imp::disable_interrupt_line::<Self>(line)
                }

                #[inline]
                unsafe fn pend_interrupt_line(
                    line: InterruptNum,
                ) -> Result<(), PendInterruptLineError> {
                    imp::pend_interrupt_line::<Self>(line)
                }

                #[inline]
                unsafe fn clear_interrupt_line(
                    line: InterruptNum,
                ) -> Result<(), ClearInterruptLineError> {
                    imp::clear_interrupt_line::<Self>(line)
                }

                #[inline]
                unsafe fn is_interrupt_line_pending(
                    line: InterruptNum,
                ) -> Result<bool, QueryInterruptLineError> {
                    imp::is_interrupt_line_pending::<Self>(line)
                }
            }

            impl InterruptController for $sys {
                #[inline]
                unsafe fn init() {
                    imp::init::<Self>()
                }

                #[inline]
                unsafe fn acknowledge_interrupt() -> Option<InterruptNum> {
                    imp::acknowledge_interrupt::<Self>()
                }

                #[inline]
                unsafe fn end_interrupt(num: InterruptNum) {
                    imp::end_interrupt::<Self>(num);
                }
            }
        };
    };
}

/// Specifies the type of signal transition that pends an interrupt.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum InterruptLineTriggerMode {
    /// Asserts an interrupt whenever the interrupt signal level is active and
    /// deasserts whenever the level is not active.
    Level = 0,
    /// Asserts an interrupt upon detection of a rising edge of an interrupt
    /// signal.
    RisingEdge = 1,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SetInterruptLineTriggerModeError {
    /// The interrupt number is out of range.
    BadParam,
}

/// The options for [`use_gic!`].
pub trait GicOptions {
    /// The base address of GIC distributor registers.
    const GIC_DISTRIBUTOR_BASE: usize;

    /// The base address of GIC CPU interface registers.
    const GIC_CPU_BASE: usize;
}

/// Provides access to a system-global GIC instance. Implemented by [`use_gic!`].
pub unsafe trait Gic {
    #[doc(hidden)]
    /// Get `GicRegs` representing the system-global GIC instance.
    fn gic_regs() -> GicRegs;

    /// Get the number of supported interrupt lines.
    fn num_interrupt_lines() -> InterruptNum {
        let distributor = Self::gic_regs().distributor;
        let raw = distributor.TYPER.read(gic_regs::GICD_TYPER::ITLinesNumber);
        (raw as usize + 1) * 32
    }

    /// Set the trigger mode of the specified interrupt line.
    fn set_interrupt_line_trigger_mode(
        num: InterruptNum,
        mode: InterruptLineTriggerMode,
    ) -> Result<(), SetInterruptLineTriggerModeError> {
        let distributor = Self::gic_regs().distributor;

        // SGI (num = `0..16`) doesn't support changing trigger mode
        if num < 16 || num >= Self::num_interrupt_lines() {
            return Err(SetInterruptLineTriggerModeError::BadParam);
        }

        let int_config = mode as u32 * 2;
        distributor.ICFGR[num / 16].modify(FieldValue::<u32, ()>::new(
            0b10,
            (num % 16) * 2,
            int_config,
        ));

        Ok(())
    }
}
