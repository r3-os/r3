#![feature(external_doc)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![doc(include = "./lib.md")]
#![no_std]

use constance::{
    kernel::{
        ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
        PendInterruptLineError, Port, PortToKernel, QueryInterruptLineError,
        SetInterruptLinePriorityError, TaskCb, UTicks,
    },
    prelude::*,
    utils::Init,
};

/// Used by `use_port!`
#[doc(hidden)]
pub extern crate constance;
/// Used by `use_port!`
#[doc(hidden)]
pub extern crate core;
/// Used by `use_port!`
#[doc(hidden)]
pub use cortex_m_rt;

/// Implemented on a system type by [`use_port!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_port!`].
#[doc(hidden)]
pub unsafe trait PortInstance: Kernel + Port<PortTaskState = TaskState> {
    fn port_state() -> &'static State;
}

#[doc(hidden)]
pub struct State {}

#[doc(hidden)]
pub struct TaskState {}

impl State {
    pub const fn new() -> Self {
        Self {}
    }
}

impl Init for TaskState {
    const INIT: Self = Self {};
}

impl State {
    pub unsafe fn port_boot<System: PortInstance>(&self) -> ! {
        // Safety: We are a port, so it's okay to call this
        unsafe {
            <System as PortToKernel>::boot();
        }
    }

    pub unsafe fn dispatch_first_task<System: PortInstance>(&'static self) -> ! {
        todo!()
    }

    pub unsafe fn yield_cpu<System: PortInstance>(&'static self) {
        todo!()
    }

    pub unsafe fn exit_and_dispatch<System: PortInstance>(
        &'static self,
        task: &'static TaskCb<System>,
    ) -> ! {
        todo!()
    }

    pub unsafe fn enter_cpu_lock<System: PortInstance>(&self) {
        // TODO: unmanaged interrupts
        cortex_m::interrupt::disable();
    }

    pub unsafe fn leave_cpu_lock<System: PortInstance>(&'static self) {
        unsafe { cortex_m::interrupt::enable() };
    }

    pub unsafe fn initialize_task_state<System: PortInstance>(
        &self,
        _task: &'static TaskCb<System>,
    ) {
        // TODO
    }

    pub fn is_cpu_lock_active<System: PortInstance>(&self) -> bool {
        todo!()
    }

    pub fn is_task_context<System: PortInstance>(&self) -> bool {
        false // TODO
    }

    pub fn set_interrupt_line_priority<System: PortInstance>(
        &'static self,
        num: InterruptNum,
        priority: InterruptPriority,
    ) -> Result<(), SetInterruptLinePriorityError> {
        todo!()
    }

    pub fn enable_interrupt_line<System: PortInstance>(
        &'static self,
        num: InterruptNum,
    ) -> Result<(), EnableInterruptLineError> {
        todo!()
    }

    pub fn disable_interrupt_line<System: PortInstance>(
        &self,
        num: InterruptNum,
    ) -> Result<(), EnableInterruptLineError> {
        todo!()
    }

    pub fn pend_interrupt_line<System: PortInstance>(
        &'static self,
        num: InterruptNum,
    ) -> Result<(), PendInterruptLineError> {
        todo!()
    }

    pub fn clear_interrupt_line<System: PortInstance>(
        &self,
        num: InterruptNum,
    ) -> Result<(), ClearInterruptLineError> {
        todo!()
    }

    pub fn is_interrupt_line_pending<System: PortInstance>(
        &self,
        num: InterruptNum,
    ) -> Result<bool, QueryInterruptLineError> {
        todo!()
    }

    pub const MAX_TICK_COUNT: UTicks = UTicks::MAX;
    pub const MAX_TIMEOUT: UTicks = UTicks::MAX / 2;
    pub fn tick_count<System: PortInstance>(&self) -> UTicks {
        0 // TODO
    }

    pub fn pend_tick_after<System: PortInstance>(&self, _tick_count_delta: UTicks) {
        // TODO
    }

    pub fn pend_tick<System: PortInstance>(&'static self) {
        // TODO
    }
}

/// Instantiate the port.
///
/// # Safety
///
///  - The target must really be a bare-metal Arm-M environment.
///  - You shouldn't interfere with the port's operrations. For example, you
///    shouldn't manually modify `FAULTMASK` or `SCB.VTOR` unless you know what
///    you are doing.
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
            use $crate::{State, TaskState, PortInstance};

            pub(super) static PORT_STATE: State = State::new();

            unsafe impl PortInstance for $sys {
                #[inline]
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

                unsafe fn enter_cpu_lock() {
                    PORT_STATE.enter_cpu_lock::<Self>()
                }

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
                    0..InterruptPriority::MAX;

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

        // TODO
        #[link_section = ".vector_table.interrupts"]
        #[no_mangle]
        static __INTERRUPTS: [usize; 1] = [0];

        #[$crate::cortex_m_rt::entry]
        fn main() -> ! {
            unsafe { port_arm_m_impl::PORT_STATE.port_boot::<$sys>() };
        }
    };
}
