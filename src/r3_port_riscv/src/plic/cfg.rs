/// The public interface of the Platform-Level Interrupt Controller driver.
use r3_core::kernel::{InterruptNum, InterruptPriority};

use super::plic_regs;

/// Implement [`InterruptController`] and [`Plic`] on the given kernel trait
/// type using the Platform-Level Interrupt Controller (PLIC) on the target.
/// **Requires [`PlicOptions`] and [`InterruptControllerToPort`].**
///
/// [`InterruptController`]: crate::InterruptController
/// [`InterruptControllerToPort`]: crate::InterruptControllerToPort
///
/// This macro adds a method `const fn configure_plic(b: &mut Cfg<C>)` to the
/// kernel trait type. **It should be called by your application's configuration
/// function.** See the following example:
///
/// ```rust,ignore
/// r3_port_riscv::use_plic!(unsafe impl InterruptController for SystemTraits);
///
/// impl r3_port_riscv::PlicOptions for SystemTraits {
///     // SiFive E
///     const MAX_PRIORITY: InterruptPriority = 7;
///     const MAX_NUM: InterruptNum = 127;
///     const PLIC_BASE: usize = 0x0c00_0000;
/// }
///
/// const fn configure_app(b: &mut r3_kernel::Cfg<SystemTraits>) -> Objects {
///     SystemTraits::configure_plic(b);
///     /* ... */
/// }
/// ```
///
/// # Safety
///
///  - The target must really include a PLIC.
///  - `PlicOptions` should be configured correctly and the memory-mapped
///    registers should be accessible.
///
#[macro_export]
macro_rules! use_plic {
    (unsafe impl InterruptController for $Traits:ty) => {
        const _: () = {
            use $crate::{
                core::ops::Range,
                plic::{imp, plic_regs},
                r3_core::kernel::{
                    traits, Cfg, ClearInterruptLineError, EnableInterruptLineError, InterruptNum,
                    InterruptPriority, PendInterruptLineError, QueryInterruptLineError,
                    SetInterruptLinePriorityError,
                },
                r3_kernel::{PortInterrupts, System},
                InterruptController, Plic, PlicOptions,
            };

            unsafe impl Plic for $Traits {
                fn plic_regs() -> &'static plic_regs::Plic {
                    unsafe { &*(<$Traits as PlicOptions>::PLIC_BASE as *const plic_regs::Plic) }
                }
            }

            impl $Traits {
                pub const fn configure_plic<C>(b: &mut Cfg<C>)
                where
                    C: ~const traits::CfgInterruptLine<System = System<Self>>,
                {
                    imp::configure::<_, Self>(b)
                }
            }

            impl InterruptController for $Traits {
                #[inline]
                unsafe fn init() {
                    imp::init::<Self>()
                }

                const MANAGED_INTERRUPT_PRIORITY_RANGE: Range<InterruptPriority> =
                    0..(<$Traits as PlicOptions>::MAX_PRIORITY + 1);

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
                    _line: InterruptNum,
                ) -> Result<(), PendInterruptLineError> {
                    Err(PendInterruptLineError::NotSupported)
                }

                #[inline]
                unsafe fn clear_interrupt_line(
                    _line: InterruptNum,
                ) -> Result<(), ClearInterruptLineError> {
                    Err(ClearInterruptLineError::NotSupported)
                }

                #[inline]
                unsafe fn is_interrupt_line_pending(
                    line: InterruptNum,
                ) -> Result<bool, QueryInterruptLineError> {
                    imp::is_interrupt_line_pending::<Self>(line)
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

    /// The last interrupt source supported by the PLIC implementation. Must be
    /// in range `0..=1023`.
    const MAX_NUM: InterruptNum;

    /// The PLIC context for the hart on which the kernel runs.
    const CONTEXT: usize = 0;

    /// Enables the trick for nested interrupt processing.
    ///
    /// PLIC is not designed to allow nested interrupt processing. When this
    /// flag is enabled, the driver will signal completion earlier to start
    /// accepting higher-priority interrupts.
    ///
    /// The following advices should be taken into consideration when enabling
    /// this option:
    ///
    ///  - This should be disabled when there is at least one interrupt source
    ///    configured to target multiple contexts.
    ///
    ///  - Some PLIC gateway implementations don't clear the pending flag when
    ///    an incoming interrupt request signal is deasserted. The pending flag
    ///    gets set again as soon as completion is signaled, meaning the
    ///    interrupt will be claimed twice every time it's taken.
    ///    The PLIC in FE310 has this issue.
    ///
    /// Defaults to `false` when unspecified.
    const USE_NESTING: bool = false;
}

/// Provides access to a system-global PLIC instance. Implemented by [`use_plic!`].
pub unsafe trait Plic: PlicOptions {
    #[doc(hidden)]
    /// Get [`plic_regs::Plic`] representing the memory-mapped interface for the
    /// PLIC instance.
    fn plic_regs() -> &'static plic_regs::Plic;
}
