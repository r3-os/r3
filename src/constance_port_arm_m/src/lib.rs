#![feature(external_doc)]
#![feature(const_fn)]
#![feature(const_mut_refs)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(llvm_asm)]
#![feature(decl_macro)]
#![feature(const_panic)]
#![feature(const_generics)]
#![feature(slice_ptr_len)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![doc(include = "./lib.md")]
#![no_std]

use constance::kernel::{InterruptNum, InterruptPriority};
use core::ops::Range;

/// Used by `use_port!`
#[doc(hidden)]
#[cfg(target_os = "none")]
pub mod threading;

/// Used by `use_port!`
#[doc(hidden)]
pub extern crate constance;
/// Used by `use_port!`
#[doc(hidden)]
#[cfg(target_os = "none")]
pub extern crate core;
/// Used by `use_port!`
#[doc(hidden)]
#[cfg(target_os = "none")]
pub use cortex_m_rt;

/// Used by `use_port!`
#[doc(hidden)]
#[cfg(target_os = "none")]
pub mod systick_tickful;

pub const INTERRUPT_PRIORITY_RANGE: Range<InterruptPriority> = 0..256;

/// `InterruptNum` for SysTick.
pub const INTERRUPT_SYSTICK: InterruptNum = 15;

/// `InterruptNum` for the first external interrupt.
pub const INTERRUPT_EXTERNAL0: InterruptNum = 16;

/// The range of valid `InterruptNum`s.
pub const INTERRUPT_NUM_RANGE: Range<InterruptNum> = 0..256;

/// The configuration of the port.
pub trait ThreadingOptions {
    /// The priority value to which CPU Lock boosts the current execution
    /// priority. Must be in range `0..256`. Defaults to `0` when unspecified.
    ///
    /// The lower bound of [`MANAGED_INTERRUPT_PRIORITY_RANGE`] is bound to this
    /// value.
    ///
    /// [`MANAGED_INTERRUPT_PRIORITY_RANGE`]: constance::kernel::PortInterrupts::MANAGED_INTERRUPT_PRIORITY_RANGE
    ///
    /// Must be `0` on Armv6-M and Armv8-M Baseline because they don't support
    /// `BASEPRI`.
    const CPU_LOCK_PRIORITY_MASK: u8 = 0;

    /// Enables the use of the `wfi` instruction in the idle task to save power.
    /// Defaults to `true`.
    const USE_WFI: bool = true;

    /// Get the top of the interrupt stack. Defaults to
    /// `*(SCB.VTOR as *const u32)`.
    ///
    /// # Safety
    ///
    /// This only can be called by the port.
    unsafe fn interrupt_stack_top() -> usize {
        #[cfg(target_os = "none")]
        {
            // Safety: We claimed the ownership of `Peripherals`
            let peripherals = unsafe { cortex_m::Peripherals::steal() };

            // Safety: `unsafe trait`
            unsafe { (peripherals.SCB.vtor.read() as *const usize).read_volatile() }
        }

        #[cfg(not(target_os = "none"))]
        panic!("unsupported target")
    }
}

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
    const TICK_PERIOD: u32 = {
        // FIXME: Work-around for `Ord::max` not being `const fn`
        let x = Self::FREQUENCY / Self::FREQUENCY_DENOMINATOR / 100;
        if x == 0 {
            1
        } else if x > 0x1000000 {
            0x1000000
        } else {
            x as u32
        }
    };
}

/// Instantiate the port.
///
/// This macro doesn't provide an implementation of [`PortTimer`], which you
/// must supply one through other ways.
/// See [the crate-level documentation](crate#kernel-timing) for possible
/// options.
///
/// [`PortTimer`]: constance::kernel::PortTimer
///
/// # Safety
///
///  - The target must really be a bare-metal Arm-M environment.
///  - You shouldn't interfere with the port's operrations. For example, you
///    shouldn't manually modify `PRIMASK` or `SCB.VTOR` unless you know what
///    you are doing.
///  - `::cortex_m_rt` should point to the `cortex-m-rt` crate.
///  - Other components should not execute the `svc` instruction.
///  - `<$sys as `[`ThreadingOptions`]`>::`[`interrupt_stack_top`] must return a
///    valid stack pointer. The default implementation evaluates `*(SCB.VTOR a
///    *const u32)`, which should be fine for most use cases, but if this is not
///    acceptable, a custom implementation should be provided.
///
/// [`interrupt_stack_top`]: ThreadingOptions::interrupt_stack_top
///
#[macro_export]
macro_rules! use_port {
    (unsafe $vis:vis struct $sys:ident) => {
        $vis struct $sys;

        mod port_arm_m_impl {
            use super::$sys;
            use $crate::constance::kernel::{
                ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
                PendInterruptLineError, Port, QueryInterruptLineError, SetInterruptLinePriorityError,
                TaskCb, PortToKernel, PortInterrupts, PortThreading, UTicks, PortTimer,
            };
            use $crate::core::ops::Range;
            use $crate::{threading::{State, TaskState, PortInstance}, ThreadingOptions};

            pub(super) static PORT_STATE: State = State::new();

            unsafe impl PortInstance for $sys {
                #[inline(always)]
                fn port_state() -> &'static State {
                    &PORT_STATE
                }
            }

            // Assume `$sys: Kernel`
            unsafe impl PortThreading for $sys {
                type PortTaskState = TaskState;
                #[allow(clippy::declare_interior_mutable_const)]
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

            unsafe impl PortInterrupts for $sys {
                const MANAGED_INTERRUPT_PRIORITY_RANGE: Range<InterruptPriority> =
                    (<$sys as ThreadingOptions>::CPU_LOCK_PRIORITY_MASK as _)..256;

                unsafe fn set_interrupt_line_priority(
                    line: InterruptNum,
                    priority: InterruptPriority,
                ) -> Result<(), SetInterruptLinePriorityError> {
                    PORT_STATE.set_interrupt_line_priority::<Self>(line, priority)
                }

                unsafe fn enable_interrupt_line(line: InterruptNum) -> Result<(), EnableInterruptLineError> {
                    PORT_STATE.enable_interrupt_line::<Self>(line)
                }

                unsafe fn disable_interrupt_line(line: InterruptNum) -> Result<(), EnableInterruptLineError> {
                    PORT_STATE.disable_interrupt_line::<Self>(line)
                }

                unsafe fn pend_interrupt_line(line: InterruptNum) -> Result<(), PendInterruptLineError> {
                    PORT_STATE.pend_interrupt_line::<Self>(line)
                }

                unsafe fn clear_interrupt_line(line: InterruptNum) -> Result<(), ClearInterruptLineError> {
                    PORT_STATE.clear_interrupt_line::<Self>(line)
                }

                unsafe fn is_interrupt_line_pending(
                    line: InterruptNum,
                ) -> Result<bool, QueryInterruptLineError> {
                    PORT_STATE.is_interrupt_line_pending::<Self>(line)
                }
            }
        }

        const _: () = $crate::threading::validate::<$sys>();

        #[link_section = ".vector_table.interrupts"]
        #[no_mangle]
        static __INTERRUPTS: $crate::threading::InterruptHandlerTable =
            $crate::threading::make_interrupt_handler_table::<$sys>();

        #[$crate::cortex_m_rt::entry]
        fn main() -> ! {
            unsafe { port_arm_m_impl::PORT_STATE.port_boot::<$sys>() };
        }

        #[$crate::cortex_m_rt::exception]
        fn PendSV() {
            unsafe { port_arm_m_impl::PORT_STATE.handle_pend_sv::<$sys>() };
        }

        #[$crate::cortex_m_rt::exception]
        fn SysTick() {
            unsafe { port_arm_m_impl::PORT_STATE.handle_sys_tick::<$sys>() };
        }
    };
}

/// Attach the tickful implementation of [`PortTimer`] that is based on SysTick
/// to a given system type.
///
/// [`PortTimer`]: constance::kernel::PortTimer
/// [a tickful scheme]: crate#tickful-systick
///
/// You should also do the following:
///
///  - Implement [`SysTickOptions`] manually.
///  - Call `$ty::configure_systick()` in your configuration function.
///    See the following example.
///
/// ```rust,ignore
/// constance_port_arm_m::use_systick_tickful!(unsafe impl PortTimer for System);
///
/// impl constance_port_arm_m::SysTickOptions for System {
///    // SysTick = AHB/8, AHB = HSI (internal 16-MHz RC oscillator)
///     const FREQUENCY: u64 = 2_000_000;
/// }
///
/// const fn configure_app(b: &mut CfgBuilder<System>) -> Objects {
///     System::configure_systick(b);
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
    (unsafe impl PortTimer for $ty:ty) => {
        const _: () = {
            use $crate::constance::{
                kernel::{cfg::CfgBuilder, PortTimer, UTicks},
                utils::Init,
            };
            use $crate::systick_tickful;

            static TIMER_STATE: systick_tickful::State<$ty> = Init::INIT;

            impl PortTimer for $ty {
                const MAX_TICK_COUNT: UTicks = u32::MAX;
                const MAX_TIMEOUT: UTicks = u32::MAX;

                unsafe fn tick_count() -> UTicks {
                    // Safety: CPU Lock active
                    unsafe { TIMER_STATE.tick_count() }
                }
            }

            // Safety: Only `use_systick_tickful!` is allowed to `impl` this
            unsafe impl systick_tickful::SysTickTickfulInstance for $ty {
                unsafe fn handle_tick() {
                    // Safety: Interrupt context, CPU Lock inactive
                    unsafe { TIMER_STATE.handle_tick::<Self>() };
                }
            }

            impl $ty {
                pub const fn configure_systick(b: &mut CfgBuilder<Self>) {
                    systick_tickful::configure(b);
                }
            }
        };
    };
}
