//! The public interface for the `mtime`-based timer driver.
use r3_core::kernel::InterruptNum;

/// Attach the implementation of [`PortTimer`] based on the RISC-V machine-mode
/// timer (`mtime`/`mtimecfg`) to a given kernel trait type. This macro also
/// implements [`Timer`] on the kernel trait type.
/// **Requires [`MtimeOptions`].**
///
/// [`PortTimer`]: r3_kernel::PortTimer
/// [`Timer`]: crate::Timer
///
/// You should do the following:
///
///  - Implement [`MtimeOptions`] on the kernel trait type `$Traits`.
///  - Call `$Traits::configure_timer()` in your configuration function.
///    See the following example.
///
/// ```rust,ignore
/// r3_port_riscv::use_mtime!(unsafe impl PortTimer for SystemTraits);
///
/// impl r3_port_riscv::MtimeOptions for SystemTraits {
///     const MTIME_PTR: usize = 0x1001_1000;
///     const MTIMECMP_PTR: usize = 0x1001_1000;
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
///  - `MtimeOptions` must be configured correctly.
///
#[macro_export]
macro_rules! use_mtime {
    (unsafe impl PortTimer for $Traits:ty) => {
        const _: () = {
            use $crate::r3_core::{
                kernel::{traits, Cfg},
                utils::Init,
            };
            use $crate::r3_kernel::{PortTimer, System, UTicks};
            use $crate::r3_portkit::tickless;
            use $crate::{mtime, MtimeOptions, Timer};

            impl PortTimer for $Traits {
                const MAX_TICK_COUNT: UTicks = u32::MAX;
                const MAX_TIMEOUT: UTicks = u32::MAX;

                unsafe fn tick_count() -> UTicks {
                    // Safety: We are just forwarding the call
                    unsafe { mtime::imp::tick_count::<Self>() }
                }

                unsafe fn pend_tick() {
                    // Safety: We are just forwarding the call
                    unsafe { mtime::imp::pend_tick::<Self>() }
                }

                unsafe fn pend_tick_after(tick_count_delta: UTicks) {
                    // Safety: We are just forwarding the call
                    unsafe { mtime::imp::pend_tick_after::<Self>(tick_count_delta) }
                }
            }

            impl Timer for $Traits {
                unsafe fn init() {
                    unsafe { mtime::imp::init::<Self>() }
                }
            }

            static mut TIMER_STATE: <$Traits as mtime::imp::TimerInstance>::TicklessState =
                Init::INIT;

            // Safety: Only `use_mtime!` is allowed to `impl` this
            unsafe impl mtime::imp::TimerInstance for $Traits {
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
                    mtime::imp::configure(b);
                }
            }
        };
    };
}

/// The options for [`use_mtime!`].
pub trait MtimeOptions {
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
    const HEADROOM: u32 =
        (Self::FREQUENCY as u128 * 60 / Self::FREQUENCY_DENOMINATOR as u128).min(0x40000000) as u32;

    /// The timer's interrupt number. Defaults to [`INTERRUPT_TIMER`].
    ///
    /// [`INTERRUPT_TIMER`]: crate::INTERRUPT_TIMER
    const INTERRUPT_NUM: InterruptNum = crate::INTERRUPT_TIMER;
}
