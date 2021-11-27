use r3::kernel::InterruptNum;

/// The interrupt number for software interrupts.
pub const INTERRUPT_SOFTWARE: InterruptNum = 0;

/// The interrupt number for timer interrupts.
pub const INTERRUPT_TIMER: InterruptNum = 1;

/// The interrupt number for external interrupts.
pub const INTERRUPT_EXTERNAL: InterruptNum = 2;

/// The first interrupt numbers allocated for use by an interrupt controller
/// driver.
pub const INTERRUPT_PLATFORM_START: InterruptNum = 3;

/// The configuration of the port.
pub trait ThreadingOptions {}

/// Define a kernel trait type implementing [`PortThreading`],
/// [`PortInterrupts`], and [`EntryPoint`].
/// **Requires [`ThreadingOptions`] and [`InterruptController`].**
///
/// [`PortThreading`]: r3::kernel::PortThreading
/// [`PortInterrupts`]: r3::kernel::PortInterrupts
/// [`EntryPoint`]: crate::EntryPoint
/// [`InterruptController`]: crate::InterruptController
#[macro_export]
macro_rules! use_port {
    (unsafe $vis:vis struct $Traits:ident) => {
        $vis struct $Traits;

        mod port_riscv_impl {
            use super::$Traits;
            use $crate::r3::kernel::{
                ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
                PendInterruptLineError, QueryInterruptLineError, SetInterruptLinePriorityError,
                interrupt::InterruptHandlerFn,
            };
            use $crate::r3_kernel::{
                TaskCb, PortToKernel, PortInterrupts, Port, PortThreading, UTicks, PortTimer,
                KernelCfg2,
            };
            use $crate::core::ops::Range;
            use $crate::{threading::imp::{State, TaskState, PortInstance}, ThreadingOptions, EntryPoint, InterruptController};

            pub(super) static PORT_STATE: State = State::new();

            unsafe impl PortInstance for $Traits {
                #[inline(always)]
                fn port_state() -> &'static State {
                    &PORT_STATE
                }

                const INTERRUPT_SOFTWARE_HANDLER: Option<InterruptHandlerFn> =
                    <$Traits as KernelCfg2>::INTERRUPT_HANDLERS.get($crate::INTERRUPT_SOFTWARE);
                const INTERRUPT_TIMER_HANDLER: Option<InterruptHandlerFn> =
                    <$Traits as KernelCfg2>::INTERRUPT_HANDLERS.get($crate::INTERRUPT_TIMER);
                const INTERRUPT_EXTERNAL_HANDLER: Option<InterruptHandlerFn> =
                    <$Traits as KernelCfg2>::INTERRUPT_HANDLERS.get($crate::INTERRUPT_EXTERNAL);
            }

            impl EntryPoint for $Traits {
                unsafe fn start() -> ! {
                    unsafe { PORT_STATE.port_boot::<Self>() };
                }

                const TRAP_HANDLER: unsafe extern "C" fn() -> ! = State::exception_handler::<Self>;
            }

            // Assume `$Traits: KernelTraits`
            unsafe impl PortThreading for $Traits {
                type PortTaskState = TaskState;
                #[allow(clippy::declare_interior_mutable_const)]
                const PORT_TASK_STATE_INIT: Self::PortTaskState =
                    $crate::r3::utils::Init::INIT;

                // The minimum stack size for all tests to pass. I found debug
                // formatting to be particularly memory-hungry.
                const STACK_DEFAULT_SIZE: usize = 512 * $crate::core::mem::size_of::<usize>();

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
                unsafe fn try_enter_cpu_lock() -> bool {
                    PORT_STATE.try_enter_cpu_lock::<Self>()
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

            unsafe impl PortInterrupts for $Traits {
                const MANAGED_INTERRUPT_PRIORITY_RANGE: Range<InterruptPriority> =
                    <$Traits as InterruptController>::MANAGED_INTERRUPT_PRIORITY_RANGE;

                const MANAGED_INTERRUPT_LINES: &'static [InterruptNum] = &[
                    $crate::INTERRUPT_SOFTWARE,
                    $crate::INTERRUPT_TIMER,
                    $crate::INTERRUPT_EXTERNAL,
                ];

                #[inline]
                unsafe fn set_interrupt_line_priority(
                    line: InterruptNum,
                    priority: InterruptPriority,
                ) -> Result<(), SetInterruptLinePriorityError> {
                    PORT_STATE.set_interrupt_line_priority::<Self>(line, priority)
                }

                #[inline]
                unsafe fn enable_interrupt_line(
                    line: InterruptNum,
                ) -> Result<(), EnableInterruptLineError> {
                    PORT_STATE.enable_interrupt_line::<Self>(line)
                }

                #[inline]
                unsafe fn disable_interrupt_line(
                    line: InterruptNum,
                ) -> Result<(), EnableInterruptLineError> {
                    PORT_STATE.disable_interrupt_line::<Self>(line)
                }

                #[inline]
                unsafe fn pend_interrupt_line(
                    line: InterruptNum,
                ) -> Result<(), PendInterruptLineError> {
                    PORT_STATE.pend_interrupt_line::<Self>(line)
                }

                #[inline]
                unsafe fn clear_interrupt_line(
                    line: InterruptNum,
                ) -> Result<(), ClearInterruptLineError> {
                    PORT_STATE.clear_interrupt_line::<Self>(line)
                }

                #[inline]
                unsafe fn is_interrupt_line_pending(
                    line: InterruptNum,
                ) -> Result<bool, QueryInterruptLineError> {
                    PORT_STATE.is_interrupt_line_pending::<Self>(line)
                }
            }
        }

        const _: () = $crate::threading::imp::validate::<$Traits>();
    };
}
