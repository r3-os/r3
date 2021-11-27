//! The public interface for the RISC-V timer.
use r3::kernel::InterruptNum;

/// Attach the implementation of [`PortTimer`] that is based on the RISC-V timer
/// (`mtime`/`mtimecfg`) to a given kernel trait type. This macro also
/// implements [`Timer`] on the system type.
/// **Requires [`TimerOptions`].**
///
/// [`PortTimer`]: r3::kernel::PortTimer
/// [`Timer`]: crate::Timer
///
/// You should do the following:
///
///  - Implement [`TimerOptions`] on the system type `$Traits`.
///  - Call `$Traits::configure_timer()` in your configuration function.
///    See the following example.
///
/// ```rust,ignore
/// r3_port_riscv::use_timer!(unsafe impl PortTimer for System);
///
/// impl r3_port_riscv::TimerOptions for System {
///     const MTIME_PTR: usize = 0x1001_1000;
///     const MTIMECMP_PTR: usize = 0x1001_1000;
///     const FREQUENCY: u64 = 1_000_000;
/// }
///
/// const fn configure_app(b: &mut CfgBuilder<System>) -> Objects {
///     System::configure_timer(b);
///     /* ... */
/// }
/// ```
///
/// # Safety
///
///  - `TimerOptions` must be configured correctly.
///
#[macro_export]
macro_rules! use_timer {
    (unsafe impl PortTimer for $Traits:ty) => {
        const _: () = {
            use $crate::r3::{
                kernel::{traits, Cfg},
                utils::Init,
            };
            use $crate::r3_kernel::{PortTimer, System, UTicks};
            use $crate::r3_portkit::tickless;
            use $crate::{timer, Timer, TimerOptions};

            impl PortTimer for $Traits {
                const MAX_TICK_COUNT: UTicks = u32::MAX;
                const MAX_TIMEOUT: UTicks = u32::MAX;

                unsafe fn tick_count() -> UTicks {
                    // Safety: We are just forwarding the call
                    unsafe { timer::imp::tick_count::<Self>() }
                }

                unsafe fn pend_tick() {
                    // Safety: We are just forwarding the call
                    unsafe { timer::imp::pend_tick::<Self>() }
                }

                unsafe fn pend_tick_after(tick_count_delta: UTicks) {
                    // Safety: We are just forwarding the call
                    unsafe { timer::imp::pend_tick_after::<Self>(tick_count_delta) }
                }
            }

            impl Timer for $Traits {
                unsafe fn init() {
                    unsafe { timer::imp::init::<Self>() }
                }
            }

            const TICKLESS_CFG: tickless::TicklessCfg =
                match tickless::TicklessCfg::new(tickless::TicklessOptions {
                    hw_freq_num: <$Traits as TimerOptions>::FREQUENCY,
                    hw_freq_denom: <$Traits as TimerOptions>::FREQUENCY_DENOMINATOR,
                    hw_headroom_ticks: <$Traits as TimerOptions>::HEADROOM,
                    // `mtime` is a 64-bit free-running counter and it is
                    // expensive to create a 32-bit timer with an arbitrary
                    // period out of it.
                    force_full_hw_period: true,
                    // If clearing `mtime` is not allowed, we must record the
                    // starting value of `mtime` by calling `reset`.
                    resettable: !<$Traits as TimerOptions>::RESET_MTIME,
                }) {
                    Ok(x) => x,
                    Err(e) => e.panic(),
                };

            static mut TIMER_STATE: tickless::TicklessState<TICKLESS_CFG> = Init::INIT;

            // Safety: Only `use_timer!` is allowed to `impl` this
            unsafe impl timer::imp::TimerInstance for $Traits {
                const TICKLESS_CFG: tickless::TicklessCfg = TICKLESS_CFG;

                type TicklessState = tickless::TicklessState<TICKLESS_CFG>;

                fn tickless_state() -> *mut Self::TicklessState {
                    unsafe { core::ptr::addr_of_mut!(TIMER_STATE) }
                }
            }

            impl $Traits {
                pub const fn configure_timer<C>(b: &mut Cfg<C>)
                where
                    C: ~const traits::CfgInterruptLine<System = System<Self>>,
                {
                    timer::imp::configure(b);
                }
            }
        };
    };
}

/// The options for [`use_timer!`].
pub trait TimerOptions {
    /// The memory address of the `mtime` register.
    const MTIME_PTR: usize;

    /// The memory address of the `mtimecmp` register.
    const MTIMECMP_PTR: usize;

    /// When set to `true`, the driver clears the lower 32 bits of the `mtime`
    /// register on boot.
    ///
    /// Disabling this might increase the runtime overhead of the driver.
    /// Nevertheless, the need to disable this might arise for numerous reasons
    /// including:
    ///
    ///  - Updating the `mtime` register [is not supported by QEMU] at this time.
    ///
    ///  - The `mtime` register might be shared with other harts and clearing it
    ///    could confuse the code running in the other harts.
    ///
    /// [is not supported by QEMU]: https://github.com/qemu/qemu/blob/672b2f2695891b6d818bddc3ce0df964c7627969/hw/riscv/sifive_clint.c#L165-L173
    const RESET_MTIME: bool = true;

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
