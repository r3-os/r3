/// The configuration of the port.
pub trait ThreadingOptions {}

/// An abstract interface to an interrupt controller. Implemented by
/// [`use_gic!`].
pub trait InterruptController {
    /// Initialize the driver. This will be called just before entering
    /// [`PortToKernel::boot`].
    ///
    /// [`PortToKernel::boot`]: r3_kernel::PortToKernel::boot
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
    unsafe fn acknowledge_interrupt() -> Option<r3_core::kernel::InterruptNum>;

    /// Notify that the kernel has completed the processing of the specified
    /// interrupt.
    ///
    /// # Safety
    ///
    /// This is only intended to be called by the port in an IRQ handler.
    unsafe fn end_interrupt(num: r3_core::kernel::InterruptNum);
}

/// An abstract inferface to a port timer driver. Implemented by
/// [`use_sp804!`].
pub trait Timer {
    /// Initialize the driver. This will be called just before entering
    /// [`PortToKernel::boot`].
    ///
    /// [`PortToKernel::boot`]: r3_kernel::PortToKernel::boot
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
    const IRQ_ENTRY: unsafe extern "C" fn() -> !;
}

/// Define a kernel trait type implementing [`PortThreading`] and
/// [`EntryPoint`]. **Requires [`ThreadingOptions`], [`InterruptController`],
/// and [`Timer`].**
///
/// [`PortThreading`]: r3_kernel::PortThreading
#[macro_export]
macro_rules! use_port {
    (unsafe $vis:vis struct $Traits:ident) => {
        $vis struct $Traits;

        mod port_arm_impl {
            use super::$Traits;
            use $crate::r3_core::kernel::{
                ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
                PendInterruptLineError, QueryInterruptLineError, SetInterruptLinePriorityError,
            };
            use $crate::r3_kernel::{
                TaskCb, PortToKernel, PortInterrupts, PortThreading, UTicks, PortTimer, Port,
            };
            use $crate::core::ops::Range;
            use $crate::threading::{
                imp::{State, TaskState, PortInstance},
                cfg::{ThreadingOptions, EntryPoint},
            };

            static PORT_STATE: State = $crate::r3_core::utils::Init::INIT;

            unsafe impl PortInstance for $Traits {
                $crate::r3_portkit::sym::sym_static!(
                    #[sym(p_port_state)] fn port_state() -> &State { &PORT_STATE });
            }

            impl EntryPoint for $Traits {
                #[inline]
                unsafe fn start() -> ! {
                    unsafe { PORT_STATE.port_boot::<Self>() }
                }

                const IRQ_ENTRY: unsafe extern "C" fn() -> ! = State::irq_entry::<Self>;
            }

            // Assume `$Traits: Kernel`
            unsafe impl PortThreading for $Traits {
                type PortTaskState = TaskState;
                #[allow(clippy::declare_interior_mutable_const)]
                const PORT_TASK_STATE_INIT: Self::PortTaskState =
                    $crate::r3_core::utils::Init::INIT;

                // The minimum stack size for all tests to pass. I found debug
                // formatting to be particularly memory-hungry.
                const STACK_DEFAULT_SIZE: usize = 2048;

                // AAPCS: "The stack must also conform to the following
                // constraint at a public interface: SP mod 8 = 0. The stack
                // must be double-word aligned."
                const STACK_ALIGN: usize = 8;

                #[inline(always)]
                unsafe fn dispatch_first_task() -> ! {
                    PORT_STATE.dispatch_first_task::<Self>()
                }

                #[inline(always)]
                unsafe fn yield_cpu() {
                    PORT_STATE.yield_cpu::<Self>()
                }

                #[inline(always)]
                unsafe fn exit_and_dispatch(task: &'static TaskCb<Self>) -> ! {
                    PORT_STATE.exit_and_dispatch::<Self>(task)
                }

                #[inline(always)]
                unsafe fn enter_cpu_lock() {
                    PORT_STATE.enter_cpu_lock::<Self>()
                }

                #[inline(always)]
                unsafe fn leave_cpu_lock() {
                    PORT_STATE.leave_cpu_lock::<Self>()
                }

                #[inline(always)]
                unsafe fn initialize_task_state(task: &'static TaskCb<Self>) {
                    PORT_STATE.initialize_task_state::<Self>(task)
                }

                #[inline(always)]
                fn is_cpu_lock_active() -> bool {
                    PORT_STATE.is_cpu_lock_active::<Self>()
                }

                #[inline(always)]
                fn is_task_context() -> bool {
                    PORT_STATE.is_task_context::<Self>()
                }

                #[inline(always)]
                fn is_interrupt_context() -> bool {
                    PORT_STATE.is_interrupt_context::<Self>()
                }

                #[inline(always)]
                fn is_scheduler_active() -> bool {
                    PORT_STATE.is_scheduler_active::<Self>()
                }
            }
        }

        const _: () = $crate::threading::imp::validate::<$Traits>();
    };
}
