#![feature(external_doc)]
#![feature(const_fn)]
#![feature(const_generics)]
#![feature(const_panic)]
#![feature(const_ptr_offset)]
#![feature(const_saturating_int_methods)]
#![feature(decl_macro)]
#![feature(llvm_asm)]
#![feature(naked_functions)]
#![feature(slice_ptr_len)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![doc(include = "./lib.md")]
#![no_std]

/// Used by `use_port!`
#[doc(hidden)]
pub extern crate constance;

#[doc(hidden)]
#[macro_use]
pub mod utils;

/// Used by `use_port!`
#[doc(hidden)]
#[cfg(target_os = "none")]
pub extern crate core;

/// Used by `use_port!`
#[doc(hidden)]
#[cfg(target_os = "none")]
pub mod threading;

/// Used by `use_sp804!`
#[doc(hidden)]
#[cfg(target_os = "none")]
pub mod sp804;

#[cfg(target_os = "none")]
mod arm;

#[doc(hidden)]
pub mod gic;
mod sp804_cfg;
mod sp804_regs;
#[doc(hidden)]
pub mod startup;
#[doc(hidden)]
pub mod timing;
pub use self::gic::cfg::*;
pub use self::sp804_cfg::*;
pub use self::startup::cfg::*;

/// The configuration of the port.
pub trait ThreadingOptions {}

/// An abstract interface to an interrupt controller. Implemented by
/// [`use_gic!`].
pub trait InterruptController {
    /// Initialize the driver. This will be called just before entering
    /// [`PortToKernel::boot`].
    ///
    /// [`PortToKernel::boot`]: constance::kernel::PortToKernel::boot
    ///
    /// # Safety
    ///
    /// This is only intended to be called by the port.
    unsafe fn init() {}

    /// Get the currently signaled interrupt and acknowledge it.
    ///
    /// # Safety
    ///
    /// This is only intended to be called by the port in an IRQ handler.
    unsafe fn acknowledge_interrupt() -> Option<constance::kernel::InterruptNum>;

    /// Notify that the kernel has completed the processing of the specified
    /// interrupt.
    ///
    /// # Safety
    ///
    /// This is only intended to be called by the port in an IRQ handler.
    unsafe fn end_interrupt(num: constance::kernel::InterruptNum);
}

/// An abstract inferface to a port timer driver. Implemented by
/// [`use_sp804!`].
pub trait Timer {
    /// Initialize the driver. This will be called just before entering
    /// [`PortToKernel::boot`].
    ///
    /// [`PortToKernel::boot`]: constance::kernel::PortToKernel::boot
    ///
    /// # Safety
    ///
    /// This is only intended to be called by the port.
    unsafe fn init() {}
}

/// Defines the entry points of a port instantiation. Implemented by
/// [`use_port!`].
pub trait EntryPoint {
    /// Proceed with the boot process.
    ///
    /// # Safety
    ///
    ///  - The processor should be in Supervisor mode.
    ///  - This method hasn't been entered yet.
    ///
    unsafe fn start() -> !;

    /// The IRQ handler.
    ///
    /// # Safety
    ///
    ///  - The processor should be in IRQ mode.
    ///  - IRQs should be masked.
    ///  - The register state of the background context should be preserved so
    ///    that the handler can restore it later.
    ///
    unsafe fn irq_entry() -> !;
}

/// Define a system type implementing [`PortThreading`] and [`EntryPoint`].
/// **Requires [`ThreadingOptions`], [`InterruptController`], and [`Timer`].**
///
/// [`PortThreading`]: constance::kernel::PortThreading
#[macro_export]
macro_rules! use_port {
    (unsafe $vis:vis struct $sys:ident) => {
        $vis struct $sys;

        mod port_arm_impl {
            use super::$sys;
            use $crate::constance::kernel::{
                ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
                PendInterruptLineError, Port, QueryInterruptLineError, SetInterruptLinePriorityError,
                TaskCb, PortToKernel, PortInterrupts, PortThreading, UTicks, PortTimer,
            };
            use $crate::core::ops::Range;
            use $crate::{threading::{State, TaskState, PortInstance}, ThreadingOptions, EntryPoint};

            pub(super) static PORT_STATE: State = State::new();

            unsafe impl PortInstance for $sys {
                #[inline(always)]
                fn port_state() -> &'static State {
                    &PORT_STATE
                }
            }

            impl EntryPoint for $sys {
                unsafe fn start() -> !{
                    unsafe { PORT_STATE.port_boot::<Self>() };
                }

                #[inline(always)]
                unsafe fn irq_entry() -> ! {
                    unsafe { State::irq_entry::<Self>() };
                }
            }

            // Assume `$sys: Kernel`
            unsafe impl PortThreading for $sys {
                type PortTaskState = TaskState;
                const PORT_TASK_STATE_INIT: Self::PortTaskState =
                    $crate::constance::utils::Init::INIT;

                // The minimum stack size for all tests to pass. I found debug
                // formatting to be particularly memory-hungry.
                const STACK_DEFAULT_SIZE: usize = 2048;

                unsafe fn dispatch_first_task() -> ! {
                    PORT_STATE.dispatch_first_task::<Self>()
                }

                unsafe fn yield_cpu() {
                    PORT_STATE.yield_cpu::<Self>()
                }

                unsafe fn exit_and_dispatch(task: &'static TaskCb<Self>) -> ! {
                    PORT_STATE.exit_and_dispatch::<Self>(task);
                }

                #[inline(always)]
                unsafe fn enter_cpu_lock() {
                    PORT_STATE.enter_cpu_lock::<Self>()
                }

                #[inline(always)]
                unsafe fn leave_cpu_lock() {
                    PORT_STATE.leave_cpu_lock::<Self>()
                }

                unsafe fn initialize_task_state(task: &'static TaskCb<Self>) {
                    PORT_STATE.initialize_task_state::<Self>(task)
                }

                fn is_cpu_lock_active() -> bool {
                    PORT_STATE.is_cpu_lock_active::<Self>()
                }

                fn is_task_context() -> bool {
                    PORT_STATE.is_task_context::<Self>()
                }
            }
        }

        const _: () = $crate::threading::validate::<$sys>();
    };
}

/// Attach the implementation of [`PortTimer`] that is based on
/// [Arm PrimeCell SP804 Dual Timer] to a given system type. This macro also
/// implements [`Timer`] on the system type.
/// **Requires [`Sp804Options`].**
///
/// [`PortTimer`]: constance::kernel::PortTimer
/// [Arm PrimeCell SP804 Dual Timer]: https://developer.arm.com/documentation/ddi0271/d/
///
/// You should do the following:
///
///  - Implement [`Sp804Options`] on the system type `$ty`.
///  - Call `$ty::configure_sp804()` in your configuration function.
///    See the following example.
///
/// ```rust,ignore
/// constance_port_arm::use_sp804!(unsafe impl PortTimer for System);
///
/// impl constance_port_arm::Sp804Options for System {
///     const SP804_BASE: usize = 0x1001_1000;
///     const FREQUENCY: u64 = 1_000_000;
///     const INTERRUPT_NUM: InterruptNum = 36;
/// }
///
/// const fn configure_app(b: &mut CfgBuilder<System>) -> Objects {
///     System::configure_sp804(b);
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
    (unsafe impl PortTimer for $ty:ty) => {
        const _: () = {
            use $crate::constance::{
                kernel::{cfg::CfgBuilder, PortTimer, UTicks},
                utils::Init,
            };
            use $crate::{sp804, timing, Sp804Options, Timer};

            impl PortTimer for $ty {
                const MAX_TICK_COUNT: UTicks = u32::MAX;
                const MAX_TIMEOUT: UTicks = u32::MAX;

                unsafe fn tick_count() -> UTicks {
                    // Safety: We are just forwarding the call
                    unsafe { sp804::tick_count::<Self>() }
                }

                unsafe fn pend_tick() {
                    // Safety: We are just forwarding the call
                    unsafe { sp804::pend_tick::<Self>() }
                }

                unsafe fn pend_tick_after(tick_count_delta: UTicks) {
                    // Safety: We are just forwarding the call
                    unsafe { sp804::pend_tick_after::<Self>(tick_count_delta) }
                }
            }

            impl Timer for $ty {
                unsafe fn init() {
                    unsafe { sp804::init::<Self>() }
                }
            }

            const TICKLESS_CFG: timing::TicklessCfg = timing::TicklessCfg::new(
                <$ty as Sp804Options>::FREQUENCY,
                <$ty as Sp804Options>::FREQUENCY_DENOMINATOR,
                <$ty as Sp804Options>::HEADROOM,
            );

            static mut TIMER_STATE: $crate::timing::TicklessState<TICKLESS_CFG> = Init::INIT;

            // Safety: Only `use_sp804!` is allowed to `impl` this
            unsafe impl sp804::Sp804Instance for $ty {
                const TICKLESS_CFG: timing::TicklessCfg = TICKLESS_CFG;

                type TicklessState = $crate::timing::TicklessState<TICKLESS_CFG>;

                fn tickless_state() -> *mut Self::TicklessState {
                    // FIXME: Use `core::ptr::raw_mut!` when it's stable
                    unsafe { &mut TIMER_STATE }
                }
            }

            impl $ty {
                pub const fn configure_sp804(b: &mut CfgBuilder<Self>) {
                    sp804::configure(b);
                }
            }
        };
    };
}
