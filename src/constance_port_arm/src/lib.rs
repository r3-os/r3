#![feature(external_doc)]
#![feature(const_fn)]
#![feature(const_panic)]
#![feature(const_ptr_offset)]
#![feature(llvm_asm)]
#![feature(naked_functions)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![doc(include = "./lib.md")]
#![no_std]

/// Used by `use_port!`
#[doc(hidden)]
pub extern crate constance;

/// Used by `use_port!`
#[doc(hidden)]
#[cfg(target_os = "none")]
pub extern crate core;

/// Used by `use_startup!`
#[doc(hidden)]
#[cfg(target_os = "none")]
pub mod startup;

/// Used by `use_port!`
#[doc(hidden)]
#[cfg(target_os = "none")]
pub mod threading;

/// Used by `use_gic!`
#[doc(hidden)]
#[cfg(target_os = "none")]
pub mod gic;

#[cfg(target_os = "none")]
mod arm;

mod startup_cfg;
pub use self::startup_cfg::*;

/// The configuration of the port.
pub trait ThreadingOptions {}

/// Defines the entry points of a port instantiation. Implemented by
/// [`use_port!`].
pub trait EntryPoint {
    /// Proceed with the boot process.
    ///
    /// # Safety
    ///
    ///  - The processor should be in System mode.
    ///  - This method hasn't been entered yet.
    ///
    unsafe fn start() -> !;
}

/// Generate [startup code]. **Requires [`StartupOptions`] and [`EntryPoint`] to
/// be implemented.**
///
/// This macro produces an entry point function whose symbol name is `start`.
/// You should specify it as an entry point in your linker script (the provided
/// linker scripts automatically do this for you).
///
/// [startup code]: crate#startup-code
#[macro_export]
macro_rules! use_startup {
    (unsafe $sys:ty) => {
        #[no_mangle]
        #[naked]
        pub unsafe fn start() {
            $crate::startup::start::<$sys>();
        }
    };
}

/// Define a system type implementing [`PortThreading`] and [`EntryPoint`].
/// **Requires [`ThreadingOptions`].**
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

/// Implement [`PortInterrupts`] on the given system type using the General
/// Interrupt Controller (GIC) on the target.
///
/// [`PortInterrupts`]: constance::kernel::PortInterrupts
///
/// # Safety
///
///  - The target must really include a GIC.
///
#[macro_export]
macro_rules! use_gic {
    (unsafe impl PortInterrupts for $sys:ty) => {
        unsafe impl $crate::constance::kernel::PortInterrupts for $sys {
            const MANAGED_INTERRUPT_PRIORITY_RANGE: $crate::core::ops::Range<
                $crate::constance::kernel::InterruptPriority,
            > = 0..255;

            unsafe fn set_interrupt_line_priority(
                _line: $crate::constance::kernel::InterruptNum,
                _priority: $crate::constance::kernel::InterruptPriority,
            ) -> Result<(), $crate::constance::kernel::SetInterruptLinePriorityError> {
                todo!()
            }

            unsafe fn enable_interrupt_line(
                _line: $crate::constance::kernel::InterruptNum,
            ) -> Result<(), $crate::constance::kernel::EnableInterruptLineError> {
                todo!()
            }

            unsafe fn disable_interrupt_line(
                _line: $crate::constance::kernel::InterruptNum,
            ) -> Result<(), $crate::constance::kernel::EnableInterruptLineError> {
                todo!()
            }

            unsafe fn pend_interrupt_line(
                _line: $crate::constance::kernel::InterruptNum,
            ) -> Result<(), $crate::constance::kernel::PendInterruptLineError> {
                todo!()
            }

            unsafe fn clear_interrupt_line(
                _line: $crate::constance::kernel::InterruptNum,
            ) -> Result<(), $crate::constance::kernel::ClearInterruptLineError> {
                todo!()
            }

            unsafe fn is_interrupt_line_pending(
                _line: $crate::constance::kernel::InterruptNum,
            ) -> Result<bool, $crate::constance::kernel::QueryInterruptLineError> {
                todo!()
            }
        }
    };
}
