//! The public interface for the SBI-based timer driver.
use r3_core::kernel::InterruptNum;

/// Attach the implementation of [`PortTimer`] based on [the RISC-V Supervisor
/// Binary Interface][1] Timer Extension (EID #0x54494D45 "TIME") and `time[h]`
/// CSR to a given kernel trait type.
/// This macro also implements [`Timer`] on the kernel trait type.
/// **Requires [`SbiTimerOptions`].**
///
/// [1]: https://github.com/riscv-non-isa/riscv-sbi-doc
/// [`PortTimer`]: r3_kernel::PortTimer
/// [`Timer`]: crate::Timer
///
/// You should do the following:
///
///  - Implement [`SbiTimerOptions`] on the kernel trait type `$Traits`.
///  - Call `$Traits::configure_timer()` in your configuration function.
///    See the following example.
///
/// ```rust,ignore
/// r3_port_riscv::use_sbi_timer!(unsafe impl PortTimer for SystemTraits);
///
/// impl r3_port_riscv::SbiTimerOptions for SystemTraits {
///     const FREQUENCY: u64 = 1_000_000;
/// }
///
/// const fn configure_app(b: &mut r3_kernel::Cfg<SystemTraits>) -> Objects {
///     SystemTraits::configure_timer(b);
///     /* ... */
/// }
/// ```
///
/// # Safety
///
///  - `SbiTimerOptions` must be configured correctly.
///
#[macro_export]
macro_rules! use_sbi_timer {
    (unsafe impl PortTimer for $Traits:ty) => {
        const _: () = {
            use $crate::r3_core::{
                kernel::{traits, Cfg},
                utils::Init,
            };
            use $crate::r3_kernel::{PortTimer, System, UTicks};
            use $crate::r3_portkit::tickless;
            use $crate::{sbi_timer, SbiTimerOptions, Timer};

            impl PortTimer for $Traits {
                const MAX_TICK_COUNT: UTicks = u32::MAX;
                const MAX_TIMEOUT: UTicks = u32::MAX;

                unsafe fn tick_count() -> UTicks {
                    // Safety: We are just forwarding the call
                    unsafe { sbi_timer::imp::tick_count::<Self>() }
                }

                unsafe fn pend_tick() {
                    // Safety: We are just forwarding the call
                    unsafe { sbi_timer::imp::pend_tick::<Self>() }
                }

                unsafe fn pend_tick_after(tick_count_delta: UTicks) {
                    // Safety: We are just forwarding the call
                    unsafe { sbi_timer::imp::pend_tick_after::<Self>(tick_count_delta) }
                }
            }

            impl Timer for $Traits {
                unsafe fn init() {
                    unsafe { sbi_timer::imp::init::<Self>() }
                }
            }

            static mut TIMER_STATE: <$Traits as sbi_timer::imp::TimerInstance>::TicklessState =
                Init::INIT;

            // Safety: Only `use_sbi_timer!` is allowed to `impl` this
            unsafe impl sbi_timer::imp::TimerInstance for $Traits {
                type TicklessState = tickless::TicklessState<{ Self::TICKLESS_CFG }>;

                fn tickless_state() -> *mut Self::TicklessState {
                    unsafe { core::ptr::addr_of_mut!(TIMER_STATE) }
                }
            }

            impl $Traits {
                pub const fn configure_timer<C>(b: &mut Cfg<C>)
                where
                    C: ~const traits::CfgInterruptLine<System = System<Self>>,
                {
                    sbi_timer::imp::configure(b);
                }
            }
        };
    };
}

/// The options for [`use_sbi_timer!`].
pub trait SbiTimerOptions {
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

    /// The timer's interrupt number. Defaults to [`INTERRUPT_TIMER`].
    ///
    /// [`INTERRUPT_TIMER`]: crate::INTERRUPT_TIMER
    const INTERRUPT_NUM: InterruptNum = crate::INTERRUPT_TIMER;
}

const fn min128(x: u128, y: u128) -> u128 {
    if x < y {
        x
    } else {
        y
    }
}
