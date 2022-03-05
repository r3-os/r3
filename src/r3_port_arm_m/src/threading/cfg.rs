use core::ops::Range;
use r3_core::kernel::{InterruptNum, InterruptPriority};

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
    /// [`MANAGED_INTERRUPT_PRIORITY_RANGE`]: r3_kernel::PortInterrupts::MANAGED_INTERRUPT_PRIORITY_RANGE
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

/// Defines the entry points of a port instantiation. Implemented by
/// [`use_port!`].
///
/// # Safety
///
/// This trait is not intended to be implemented in any other means.
pub unsafe trait EntryPoint {
    /// Proceed with the boot process.
    ///
    /// # Safety
    ///
    ///  - The processor should be in Thread mode.
    ///  - This method hasn't been entered yet.
    ///
    unsafe fn start() -> !;

    /// The PendSV handler.
    ///
    /// # Safety
    ///
    ///  - This method must be registered as a PendSV handler. The callee-saved
    ///    registers must contain the values from the background context.
    ///
    const HANDLE_PEND_SV: unsafe extern "C" fn();
}

/// Instantiate the port. Implements the port traits ([`PortThreading`], etc.)
/// and [`EntryPoint`].
///
/// This macro doesn't provide an implementation of [`PortTimer`], which you
/// must supply one through other ways.
/// See [the crate-level documentation](crate#kernel-timing) for possible
/// options.
///
/// [`PortThreading`]: r3_kernel::PortThreading
/// [`PortTimer`]: r3_kernel::PortTimer
///
/// # Safety
///
///  - The target must really be a bare-metal Arm-M environment.
///  - You shouldn't interfere with the port's operrations. For example, you
///    shouldn't manually modify `PRIMASK` or `SCB.VTOR` unless you know what
///    you are doing.
///  - `::cortex_m_rt` should point to the `cortex-m-rt` crate.
///  - Other components should not execute the `svc` instruction.
///  - `<$Traits as `[`ThreadingOptions`]`>::`[`interrupt_stack_top`] must
///    return a valid stack pointer. The default implementation evaluates
///    `*(SCB.VTOR a *const u32)`, which should be fine for most use cases, but
///    if this is not acceptable, a custom implementation should be provided.
///
/// [`interrupt_stack_top`]: ThreadingOptions::interrupt_stack_top
///
#[macro_export]
macro_rules! use_port {
    (unsafe $vis:vis struct $Traits:ident) => {
        $vis struct $Traits;

        mod port_arm_m_impl {
            use super::$Traits;
            use $crate::r3_core::kernel::{
                ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
                PendInterruptLineError, QueryInterruptLineError, SetInterruptLinePriorityError,
            };
            use $crate::r3_kernel::{
                Port, TaskCb, PortToKernel, PortInterrupts, PortThreading, UTicks, PortTimer,
            };
            use $crate::core::ops::Range;
            use $crate::threading::{
                imp::{State, TaskState, PortInstance},
                cfg::{ThreadingOptions, EntryPoint},
            };

            #[inline(always)]
            pub(super) fn port_state() -> &'static State {
                <$Traits as PortInstance>::port_state()
            }

            unsafe impl PortInstance for $Traits {}

            // Assume `$Traits: KernelTraits`
            unsafe impl PortThreading for $Traits {
                type PortTaskState = TaskState;
                #[allow(clippy::declare_interior_mutable_const)]
                const PORT_TASK_STATE_INIT: Self::PortTaskState =
                    $crate::r3_core::utils::Init::INIT;

                // The minimum stack size for all tests to pass. I found debug
                // formatting to be particularly memory-hungry.
                const STACK_DEFAULT_SIZE: usize = 2048;

                // Armv8-M ARM: "R_LDGJ: On exception return the Armv8-M
                // architecture only supports doubleword aligned stack
                // pointers."
                //
                // AAPCS: "The stack must also conform to the following
                // constraint at a public interface: SP mod 8 = 0. The stack
                // must be double-word aligned."
                const STACK_ALIGN: usize = 8;

                #[inline(always)]
                unsafe fn dispatch_first_task() -> ! {
                    port_state().dispatch_first_task::<Self>()
                }

                #[inline(always)]
                unsafe fn yield_cpu() {
                    port_state().yield_cpu::<Self>()
                }

                #[inline(always)]
                unsafe fn exit_and_dispatch(task: &'static TaskCb<Self>) -> ! {
                    port_state().exit_and_dispatch::<Self>(task);
                }

                #[inline(always)]
                unsafe fn enter_cpu_lock() {
                    port_state().enter_cpu_lock::<Self>()
                }

                #[inline(always)]
                unsafe fn leave_cpu_lock() {
                    port_state().leave_cpu_lock::<Self>()
                }

                #[inline(always)]
                unsafe fn initialize_task_state(task: &'static TaskCb<Self>) {
                    port_state().initialize_task_state::<Self>(task)
                }

                #[inline(always)]
                fn is_cpu_lock_active() -> bool {
                    port_state().is_cpu_lock_active::<Self>()
                }

                #[inline(always)]
                fn is_task_context() -> bool {
                    port_state().is_task_context::<Self>()
                }

                #[inline(always)]
                fn is_interrupt_context() -> bool {
                    port_state().is_interrupt_context::<Self>()
                }

                #[inline(always)]
                fn is_scheduler_active() -> bool {
                    port_state().is_scheduler_active::<Self>()
                }
            }

            unsafe impl PortInterrupts for $Traits {
                const MANAGED_INTERRUPT_PRIORITY_RANGE: Range<InterruptPriority> =
                    (<$Traits as ThreadingOptions>::CPU_LOCK_PRIORITY_MASK as _)..256;

                unsafe fn set_interrupt_line_priority(
                    line: InterruptNum,
                    priority: InterruptPriority,
                ) -> Result<(), SetInterruptLinePriorityError> {
                    port_state().set_interrupt_line_priority::<Self>(line, priority)
                }

                unsafe fn enable_interrupt_line(line: InterruptNum) -> Result<(), EnableInterruptLineError> {
                    port_state().enable_interrupt_line::<Self>(line)
                }

                unsafe fn disable_interrupt_line(line: InterruptNum) -> Result<(), EnableInterruptLineError> {
                    port_state().disable_interrupt_line::<Self>(line)
                }

                unsafe fn pend_interrupt_line(line: InterruptNum) -> Result<(), PendInterruptLineError> {
                    port_state().pend_interrupt_line::<Self>(line)
                }

                unsafe fn clear_interrupt_line(line: InterruptNum) -> Result<(), ClearInterruptLineError> {
                    port_state().clear_interrupt_line::<Self>(line)
                }

                unsafe fn is_interrupt_line_pending(
                    line: InterruptNum,
                ) -> Result<bool, QueryInterruptLineError> {
                    port_state().is_interrupt_line_pending::<Self>(line)
                }
            }

            unsafe impl EntryPoint for $Traits {
                #[inline]
                unsafe fn start() -> ! {
                    unsafe { port_state().port_boot::<$Traits>() }
                }

                const HANDLE_PEND_SV: unsafe extern "C" fn() =
                    State::handle_pend_sv::<$Traits>;
            }
        }

        const _: () = $crate::threading::imp::validate::<$Traits>();
    };
}
