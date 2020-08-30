//! The public interface for the RZ/A1 OS Timer driver.
use constance::kernel::{InterruptNum, InterruptPriority};

/// Attach the implementation of [`PortTimer`] that is based on RZ/A1 OS Timer
/// to a given system type. This macro also implements [`Timer`] on the system
/// type.
/// **Requires [`OsTimerOptions`] and [`Gic`].**
///
/// [`PortTimer`]: constance::kernel::PortTimer
/// [`Timer`]: constance_port_arm::Timer
/// [`Gic`]: constance_port_arm::Gic
///
/// You should do the following:
///
///  - Implement [`OsTimerOptions`] on the system type `$ty`.
///  - Call `$ty::configure_os_timer()` in your configuration function.
///    See the following example.
///
/// ```rust,ignore
/// constance_support_rza1::use_os_timer!(unsafe impl PortTimer for System);
///
/// impl constance_support_rza1::OsTimerOptions for System {
///     const FREQUENCY: u64 = 1_000_000;
/// }
///
/// const fn configure_app(b: &mut CfgBuilder<System>) -> Objects {
///     System::configure_os_timer(b);
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
    (unsafe impl PortTimer for $ty:ty) => {
        const _: () = {
            use $crate::constance::{
                kernel::{cfg::CfgBuilder, PortTimer, UTicks},
                utils::Init,
            };
            use $crate::constance_port_arm::Timer;
            use $crate::constance_portkit::tickless;
            use $crate::{os_timer, OsTimerOptions};

            impl PortTimer for $ty {
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

            impl Timer for $ty {
                unsafe fn init() {
                    unsafe { os_timer::imp::init::<Self>() }
                }
            }

            const TICKLESS_CFG: tickless::TicklessCfg =
                match tickless::TicklessCfg::new(tickless::TicklessOptions {
                    hw_freq_num: <$ty as OsTimerOptions>::FREQUENCY,
                    hw_freq_denom: <$ty as OsTimerOptions>::FREQUENCY_DENOMINATOR,
                    hw_headroom_ticks: <$ty as OsTimerOptions>::HEADROOM,
                    force_full_hw_period: true,
                    resettable: false,
                }) {
                    Ok(x) => x,
                    Err(e) => e.panic(),
                };

            static mut TIMER_STATE: tickless::TicklessState<TICKLESS_CFG> = Init::INIT;

            // Safety: Only `use_os_timer!` is allowed to `impl` this
            unsafe impl os_timer::imp::OsTimerInstance for $ty {
                const TICKLESS_CFG: tickless::TicklessCfg = TICKLESS_CFG;

                type TicklessState = tickless::TicklessState<TICKLESS_CFG>;

                fn tickless_state() -> *mut Self::TicklessState {
                    // FIXME: Use `core::ptr::raw_mut!` when it's stable
                    unsafe { &mut TIMER_STATE }
                }
            }

            impl $ty {
                pub const fn configure_os_timer(b: &mut CfgBuilder<Self>) {
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
