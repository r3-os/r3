/// The public interface of the Platform-Level Interrupt Controller driver.
use constance::kernel::InterruptPriority;

/// Implement [`PortInterrupts`], [`InterruptController`], and [`Plic`] on
/// the given system type using the Platform-Level Interrupt Controller (PLIC)
/// on the target.
/// **Requires [`PlicOptions`].**
///
/// [`PortInterrupts`]: constance::kernel::PortInterrupts
/// [`InterruptController`]: crate::InterruptController
///
/// # Safety
///
///  - The target must really include a PLIC.
///  - `PlicOptions` should be configured correctly and the memory-mapped
///    registers should be accessible.
///
#[macro_export]
macro_rules! use_plic {
    (unsafe impl PortInterrupts for $sys:ty) => {
        const _: () = {
            use $crate::{
                constance::kernel::{
                    ClearInterruptLineError, EnableInterruptLineError, InterruptNum,
                    InterruptPriority, PendInterruptLineError, PortInterrupts,
                    QueryInterruptLineError, SetInterruptLinePriorityError,
                },
                core::ops::Range,
                plic::imp,
                InterruptController, Plic,
            };

            unsafe impl Plic for $sys {}

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
                type Token = u32;

                #[inline]
                unsafe fn init() {
                    imp::init::<Self>()
                }

                #[inline]
                unsafe fn claim_interrupt() -> Option<(u32, InterruptNum)> {
                    imp::claim_interrupt::<Self>()
                }

                #[inline]
                unsafe fn end_interrupt(token: Self::Token) {
                    imp::end_interrupt::<Self>(token);
                }
            }
        };
    };
}

/// The options for [`use_plic!`].
pub trait PlicOptions {
    /// The base address of PLIC's memory-mapped registers.
    const PLIC_BASE: usize;

    /// The maximum (highest) interrupt priority supported by the PLIC
    /// implementation.
    const MAX_PRIORITY: InterruptPriority;
}

/// Provides access to a system-global PLIC instance. Implemented by [`use_plic!`].
pub unsafe trait Plic {
    // TODO
}
