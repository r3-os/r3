#![feature(external_doc)]
#![feature(const_fn)]
#![feature(llvm_asm)]
#![feature(const_panic)]
#![feature(slice_ptr_len)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![doc(include = "./lib.md")]
#![no_std]
#![cfg(any(target_os = "none", doc))]

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

pub const INTERRUPT_PRIORITY_RANGE: Range<InterruptPriority> = 0..256;

/// `InterruptNum` for SysTick.
pub const INTERRUPT_SYSTICK: InterruptNum = 15;

/// `InterruptNum` for the first external interrupt.
pub const INTERRUPT_EXTERNAL0: InterruptNum = 16;

/// The range of valid `InterruptNum`s.
pub const INTERRUPT_NUM_RANGE: Range<InterruptNum> = 0..256;

/// The configuration of the port.
///
/// # Safety
///
///  - `interrupt_stack_top` must return a valid stack pointer. The default
///    implementation evaluates `*(SCB.VTOR as *const u32)`, which should be
///    fine for most use cases, but if this is not acceptable, a custom
///    implementation should be provided.
///
pub unsafe trait PortCfg {
    /// The priority value to which CPU Lock boosts the current execution
    /// priority. Must be in range `0..256`. Defaults to `0` when unspecified.
    ///
    /// The lower bound of [`MANAGED_INTERRUPT_PRIORITY_RANGE`] is bound to this
    /// value.
    ///
    /// [`MANAGED_INTERRUPT_PRIORITY_RANGE`]: constance::kernel::PortInterrupts::MANAGED_INTERRUPT_PRIORITY_RANGE
    ///
    /// Must be `0` on an Armv6-M target because it doesn't support `BASEPRI`.
    const CPU_LOCK_PRIORITY_MASK: u8 = 0;

    /// Get the top of the interrupt stack. Defaults to
    /// `*(SCB.VTOR as *const u32)`.
    ///
    /// # Safety
    ///
    /// This only can be called by this crate's implementation of
    /// [`dispatch_first_task`].
    ///
    /// [`dispatch_first_task`]: constance::kernel::PortThreading::dispatch_first_task
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

/// Instantiate the port.
///
/// # Safety
///
///  - The target must really be a bare-metal Arm-M environment.
///  - You shouldn't interfere with the port's operrations. For example, you
///    shouldn't manually modify `PRIMASK` or `SCB.VTOR` unless you know what
///    you are doing.
///  - `::cortex_m_rt` should point to the `cortex-m-rt` crate.
///  - Other components should not execute the `svc` instruction.
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
            use $crate::{threading::{State, TaskState, PortInstance}, PortCfg};

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
                const PORT_TASK_STATE_INIT: Self::PortTaskState =
                    $crate::constance::utils::Init::INIT;

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
                    (<$sys as PortCfg>::CPU_LOCK_PRIORITY_MASK as _)..256;

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

            impl PortTimer for $sys {
                const MAX_TICK_COUNT: UTicks = State::MAX_TICK_COUNT;
                const MAX_TIMEOUT: UTicks = State::MAX_TIMEOUT;

                unsafe fn tick_count() -> UTicks {
                    PORT_STATE.tick_count::<Self>()
                }

                unsafe fn pend_tick_after(tick_count_delta: UTicks) {
                    PORT_STATE.pend_tick_after::<Self>(tick_count_delta)
                }

                unsafe fn pend_tick() {
                    PORT_STATE.pend_tick::<Self>()
                }
            }
        }

        #[link_section = ".vector_table.interrupts"]
        #[no_mangle]
        static __INTERRUPTS: $crate::threading::InterruptHandlerTable =
            $crate::threading::make_interrupt_handler_table::<$sys>();

        #[$crate::cortex_m_rt::entry]
        fn main() -> ! {
            unsafe { port_arm_m_impl::PORT_STATE.port_boot::<$sys>() };
        }

        #[$crate::cortex_m_rt::exception]
        fn SVCall() {
            unsafe { port_arm_m_impl::PORT_STATE.handle_sv_call::<$sys>() };
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
