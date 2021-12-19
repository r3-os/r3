//! The public interface for the SP804 Dual Timer driver.
use r3::kernel::{InterruptNum, InterruptPriority};

/// Attach the implementation of [`PortTimer`] that is based on
/// [Arm PrimeCell SP804 Dual Timer] to a given kernel trait type. This macro
/// also implements [`Timer`] on the kernel trait type.
/// **Requires [`Sp804Options`].**
///
/// [`PortTimer`]: r3_kernel::PortTimer
/// [`Timer`]: crate::Timer
/// [Arm PrimeCell SP804 Dual Timer]: https://developer.arm.com/documentation/ddi0271/d/
///
/// You should do the following:
///
///  - Implement [`Sp804Options`] on the kernel trait type `$Traits`.
///  - Call `$Traits::configure_sp804()` in your configuration function.
///    See the following example.
///
/// ```rust,ignore
/// r3_port_arm::use_sp804!(unsafe impl PortTimer for SystemTraits);
///
/// impl r3_port_arm::Sp804Options for SystemTraits {
///     const SP804_BASE: usize = 0x1001_1000;
///     const FREQUENCY: u64 = 1_000_000;
///     const INTERRUPT_NUM: InterruptNum = 36;
/// }
///
/// const fn configure_app<C>(b: &mut Cfg<SystemTraits>) -> Objects
/// where
///     C: ~const traits::CfgBase<System = System<SystemTraits>>,
/// {
///     SystemTraits::configure_sp804(b);
///     /* ... */
/// }
/// ```
///
/// # Safety
///
///  - `Sp804Options` must be configured correctly.
///
#[macro_export]
macro_rules! use_sp804 {
    (unsafe impl PortTimer for $Traits:ty) => {
        const _: () = {
            use $crate::r3::{
                kernel::{traits, Cfg},
                utils::ConstDefault,
            };
            use $crate::r3_kernel::{PortTimer, System, UTicks};
            use $crate::r3_portkit::tickless;
            use $crate::{sp804, Sp804Options, Timer};

            impl PortTimer for $Traits {
                const MAX_TICK_COUNT: UTicks = u32::MAX;
                const MAX_TIMEOUT: UTicks = u32::MAX;

                unsafe fn tick_count() -> UTicks {
                    // Safety: We are just forwarding the call
                    unsafe { sp804::imp::tick_count::<Self>() }
                }

                unsafe fn pend_tick() {
                    // Safety: We are just forwarding the call
                    unsafe { sp804::imp::pend_tick::<Self>() }
                }

                unsafe fn pend_tick_after(tick_count_delta: UTicks) {
                    // Safety: We are just forwarding the call
                    unsafe { sp804::imp::pend_tick_after::<Self>(tick_count_delta) }
                }
            }

            impl Timer for $Traits {
                unsafe fn init() {
                    unsafe { sp804::imp::init::<Self>() }
                }
            }

            const TICKLESS_CFG: tickless::TicklessCfg =
                match tickless::TicklessCfg::new(tickless::TicklessOptions {
                    hw_freq_num: <$Traits as Sp804Options>::FREQUENCY,
                    hw_freq_denom: <$Traits as Sp804Options>::FREQUENCY_DENOMINATOR,
                    hw_headroom_ticks: <$Traits as Sp804Options>::HEADROOM,
                    force_full_hw_period: false,
                    resettable: false,
                }) {
                    Ok(x) => x,
                    Err(e) => e.panic(),
                };

            static mut TIMER_STATE: tickless::TicklessState<TICKLESS_CFG> = ConstDefault::DEFAULT;

            // Safety: Only `use_sp804!` is allowed to `impl` this
            unsafe impl sp804::imp::Sp804Instance for $Traits {
                const TICKLESS_CFG: tickless::TicklessCfg = TICKLESS_CFG;

                type TicklessState = tickless::TicklessState<TICKLESS_CFG>;

                fn tickless_state() -> *mut Self::TicklessState {
                    unsafe { core::ptr::addr_of_mut!(TIMER_STATE) }
                }
            }

            impl $Traits {
                pub const fn configure_sp804<C>(b: &mut Cfg<C>)
                where
                    C: ~const traits::CfgInterruptLine<System = System<Self>>,
                {
                    sp804::imp::configure(b);
                }
            }
        };
    };
}

/// The options for [`use_sp804!`].
pub trait Sp804Options {
    /// The base address of SP804's memory-mapped registers.
    const SP804_BASE: usize;

    /// The numerator of the effective timer clock rate of the dual timer.
    const FREQUENCY: u64;

    /// The denominator of the effective timer clock rate of the dual timer.
    /// Defaults to `1`.
    const FREQUENCY_DENOMINATOR: u64 = 1;

    /// The maximum permissible timer interrupt latency, measured in hardware
    /// timer cycles.
    ///
    /// Defaults to `min(FREQUENCY * 60 / FREQUENCY_DENOMINATOR, 0x40000000)`.
    const HEADROOM: u32 = min128(
        Self::FREQUENCY as u128 * 60 / Self::FREQUENCY_DENOMINATOR as u128,
        0x40000000,
    ) as u32;

    /// The interrupt priority of the timer interrupt line.
    /// Defaults to `0xc0`.
    const INTERRUPT_PRIORITY: InterruptPriority = 0xc0;

    /// The timer's interrupt number.
    const INTERRUPT_NUM: InterruptNum;
}

const fn min128(x: u128, y: u128) -> u128 {
    if x < y {
        x
    } else {
        y
    }
}
