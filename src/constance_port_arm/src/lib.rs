#![feature(external_doc)]
#![feature(const_fn)]
#![feature(const_generics)]
#![feature(const_panic)]
#![feature(const_ptr_offset)]
#![feature(decl_macro)]
#![feature(asm)]
#![feature(naked_functions)]
#![feature(slice_ptr_len)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(clippy::verbose_bit_mask)] // questionable
#![doc(include = "./lib.md")]
#![no_std]

/// Used by `use_port!`
#[doc(hidden)]
pub extern crate constance;

/// Used by `use_sp804!`
#[doc(hidden)]
pub extern crate constance_portkit;

/// Used by `use_port!`
#[doc(hidden)]
#[cfg(target_os = "none")]
pub extern crate core;

/// Used by `use_port!`
#[doc(hidden)]
#[cfg(target_os = "none")]
pub mod threading;

#[cfg(target_os = "none")]
mod arm;

/// The Arm Generic Interrupt Controller driver.
#[doc(hidden)]
pub mod gic {
    pub mod cfg;
    mod gic_regs;
    pub mod imp;
}

/// The standard startup code.
#[doc(hidden)]
pub mod startup {
    pub mod cfg;
    #[cfg(target_os = "none")]
    pub mod imp;
}

/// The SP804 Dual Timer driver.
#[doc(hidden)]
pub mod sp804 {
    pub mod cfg;
    pub mod imp;
    mod sp804_regs;
}

pub use self::gic::cfg::*;
pub use self::sp804::cfg::*;
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
