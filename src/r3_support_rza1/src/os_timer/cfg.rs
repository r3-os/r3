//! The public interface for the RZ/A1 OS Timer driver.
use r3::kernel::{InterruptNum, InterruptPriority};

/// Attach the implementation of [`PortTimer`] that is based on RZ/A1 OS Timer
/// to a given kernel trait type. This macro also implements [`Timer`] on the
/// kernel trait type.
/// **Requires [`OsTimerOptions`] and [`Gic`].**
///
/// [`PortTimer`]: r3::kernel::PortTimer
/// [`Timer`]: r3_port_arm::Timer
/// [`Gic`]: r3_port_arm::Gic
///
/// You should do the following:
///
///  - Implement [`OsTimerOptions`] on the kernel trait type `$Traits`.
///  - Call `$Traits::configure_os_timer()` in your configuration function.
///    See the following example.
///
/// ```rust,ignore
/// r3_support_rza1::use_os_timer!(unsafe impl PortTimer for SystemTraits);
///
/// impl r3_support_rza1::OsTimerOptions for SystemTraits {
///     const FREQUENCY: u64 = 1_000_000;
/// }
///
/// const fn configure_app(b: &mut Cfg<C>) -> Objects
/// where
///     C: ~const traits::CfgBase<System = System<SystemTraits>>,
/// {
///     SystemTraits::configure_os_timer(b);
///     /* ... */
/// }
/// ```
///
/// # Safety
///
///  - `OsTimerOptions` must be configured correctly.
///
#[macro_export]
macro_rules! use_os_timer {
    (unsafe impl PortTimer for $Traits:ty) => {
        const _: () = {
            use $crate::r3::{
                kernel::{traits, Cfg},
                utils::Init,
            };
            use $crate::r3_kernel::{PortTimer, System, UTicks};
            use $crate::r3_port_arm::Timer;
            use $crate::r3_portkit::tickless;
            use $crate::{os_timer, OsTimerOptions};

            impl PortTimer for $Traits {
                const MAX_TICK_COUNT: UTicks = u32::MAX;
                const MAX_TIMEOUT: UTicks = u32::MAX;

                unsafe fn tick_count() -> UTicks {
                    // Safety: We are just forwarding the call
                    unsafe { os_timer::imp::tick_count::<Self>() }
                }

                unsafe fn pend_tick() {
                    // Safety: We are just forwarding the call
                    unsafe { os_timer::imp::pend_tick::<Self>() }
                }

                unsafe fn pend_tick_after(tick_count_delta: UTicks) {
                    // Safety: We are just forwarding the call
                    unsafe { os_timer::imp::pend_tick_after::<Self>(tick_count_delta) }
                }
            }

            impl Timer for $Traits {
                unsafe fn init() {
                    unsafe { os_timer::imp::init::<Self>() }
                }
            }

            const TICKLESS_CFG: tickless::TicklessCfg =
                match tickless::TicklessCfg::new(tickless::TicklessOptions {
                    hw_freq_num: <$Traits as OsTimerOptions>::FREQUENCY,
                    hw_freq_denom: <$Traits as OsTimerOptions>::FREQUENCY_DENOMINATOR,
                    hw_headroom_ticks: <$Traits as OsTimerOptions>::HEADROOM,
                    force_full_hw_period: true,
                    resettable: false,
                }) {
                    Ok(x) => x,
                    Err(e) => e.panic(),
                };

            static mut TIMER_STATE: tickless::TicklessState<TICKLESS_CFG> = Init::INIT;

            // Safety: Only `use_os_timer!` is allowed to `impl` this
            unsafe impl os_timer::imp::OsTimerInstance for $Traits {
                const TICKLESS_CFG: tickless::TicklessCfg = TICKLESS_CFG;

                type TicklessState = tickless::TicklessState<TICKLESS_CFG>;

                fn tickless_state() -> *mut Self::TicklessState {
                    unsafe { core::ptr::addr_of_mut!(TIMER_STATE) }
                }
            }

            impl $Traits {
                pub const fn configure_os_timer<C>(b: &mut Cfg<C>)
                where
                    C: ~const traits::CfgInterruptLine<System = System<Self>>,
                {
                    os_timer::imp::configure(b);
                }
            }
        };
    };
}

/// The options for [`use_os_timer!`].
pub trait OsTimerOptions {
    /// The base address of OS Timer's memory-mapped registers.
    const OSTM_BASE: usize = 0xfcfec000;

    /// The standby control register's memory address and bit position used to
    /// enable the clock supply to OS Timer.
    ///
    /// Defaults to `Some((0xfcfe0428, 1))` (STBCR5.MSTP51).
    const STBCR_OSTM: Option<(usize, u8)> = Some((0xfcfe0428, 1));

    /// The numerator of the timer clock rate of the timer.
    const FREQUENCY: u64;

    /// The denominator of the timer clock rate of the timer.
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
    const INTERRUPT_OSTM_PRIORITY: InterruptPriority = 0xc0;

    /// OS Timer's interrupt number.
    const INTERRUPT_OSTM: InterruptNum = 134;
}

const fn min128(x: u128, y: u128) -> u128 {
    if x < y {
        x
    } else {
        y
    }
}
