/// The configuration of the port.
pub trait ThreadingOptions {}

/// Define a system type implementing [`PortThreading`] and [`EntryPoint`].
/// **Requires [`ThreadingOptions`].**
///
/// [`PortThreading`]: constance::kernel::PortThreading
/// [`EntryPoint`]: crate::EntryPoint
#[macro_export]
macro_rules! use_port {
    (unsafe $vis:vis struct $sys:ident) => {
        $vis struct $sys;

        mod port_riscv_impl {
            use super::$sys;
            use $crate::constance::kernel::{
                ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
                PendInterruptLineError, Port, QueryInterruptLineError, SetInterruptLinePriorityError,
                TaskCb, PortToKernel, PortInterrupts, PortThreading, UTicks, PortTimer,
            };
            use $crate::core::ops::Range;
            use $crate::{threading::imp::{State, TaskState, PortInstance}, ThreadingOptions, EntryPoint};

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

        const _: () = $crate::threading::imp::validate::<$sys>();
    };
}
