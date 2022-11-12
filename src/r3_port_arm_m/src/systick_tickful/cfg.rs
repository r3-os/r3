use r3_core::kernel::InterruptPriority;

/// The configuration for the implementation of `PortTimer` based on SysTick
/// ([tickful]).
///
/// [tickful]: crate::use_systick_tickful
pub trait SysTickOptions {
    /// The numerator of the input clock frequency of SysTick.
    const FREQUENCY: u64;

    /// The denominator of the input clock frequency of SysTick.
    /// Defaults to `1`.
    const FREQUENCY_DENOMINATOR: u64 = 1;

    /// The interrupt priority of the SysTick interrupt line.
    /// Defaults to `0xc0`.
    const INTERRUPT_PRIORITY: InterruptPriority = 0xc0;

    /// The period of ticks, measured in SysTick cycles. Must be in range
    /// `0..=0x1000000`.
    ///
    /// Defaults to
    /// `(FREQUENCY / FREQUENCY_DENOMINATOR / 100).max(1).min(0x1000000)` (100Hz).
    const TICK_PERIOD: u32 =
        (Self::FREQUENCY / Self::FREQUENCY_DENOMINATOR / 100).clamp(0, 0x1000000) as u32;
}

/// Attach the tickful implementation of [`PortTimer`] that is based on SysTick
/// to a given kernel trait type.
///
/// [`PortTimer`]: r3_kernel::PortTimer
/// [a tickful scheme]: crate#tickful-systick
///
/// You should also do the following:
///
///  - Implement [`SysTickOptions`] manually.
///  - Call `$Traits::configure_systick()` in your configuration function.
///    See the following example.
///
/// ```rust,ignore
/// r3_port_arm_m::use_systick_tickful!(unsafe impl PortTimer for SystemTraits);
///
/// impl r3_port_arm_m::SysTickOptions for SystemTraits {
///    // SysTick = AHB/8, AHB = HSI (internal 16-MHz RC oscillator)
///     const FREQUENCY: u64 = 2_000_000;
/// }
///
/// const fn configure_app(b: &mut r3_kernel::Cfg<SystemTraits>) -> Objects {
///     SystemTraits::configure_systick(b);
///     /* ... */
/// }
/// ```
///
/// # Safety
///
///  - The target must really be a bare-metal Arm-M environment.
///
#[macro_export]
macro_rules! use_systick_tickful {
    (unsafe impl PortTimer for $Traits:ty) => {
        const _: () = {
            use $crate::r3_core::{
                kernel::{traits, Cfg},
                utils::Init,
            };
            use $crate::r3_kernel::{PortTimer, System, UTicks};
            use $crate::systick_tickful::imp;

            static TIMER_STATE: imp::State<$Traits> = Init::INIT;

            impl PortTimer for $Traits {
                const MAX_TICK_COUNT: UTicks = u32::MAX;
                const MAX_TIMEOUT: UTicks = u32::MAX;

                #[inline(always)]
                unsafe fn tick_count() -> UTicks {
                    // Safety: CPU Lock active
                    unsafe { TIMER_STATE.tick_count() }
                }
            }

            // Safety: Only `use_systick_tickful!` is allowed to `impl` this
            unsafe impl imp::SysTickTickfulInstance for $Traits {
                #[inline(always)]
                unsafe fn handle_tick() {
                    // Safety: Interrupt context, CPU Lock inactive
                    unsafe { TIMER_STATE.handle_tick::<Self>() };
                }
            }

            impl $Traits {
                pub const fn configure_systick<C>(b: &mut Cfg<C>)
                where
                    C: ~const traits::CfgInterruptLine<System = System<Self>>,
                {
                    imp::configure(b);
                }
            }
        };
    };
}
