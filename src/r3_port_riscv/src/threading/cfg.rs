use r3_core::kernel::InterruptNum;

/// The interrupt number for software interrupts.
pub const INTERRUPT_SOFTWARE: InterruptNum = 0;

/// The interrupt number for timer interrupts.
pub const INTERRUPT_TIMER: InterruptNum = 1;

/// The interrupt number for external interrupts.
pub const INTERRUPT_EXTERNAL: InterruptNum = 2;

/// The first interrupt number allocated for use by an interrupt controller
/// driver.
pub const INTERRUPT_PLATFORM_START: InterruptNum = 3;

/// The configuration of the port.
pub trait ThreadingOptions {
    /// The RISC-V privilege level wherein the kernel and apllication operate.
    /// The default value is [`PRIVILEGE_LEVEL_MACHINE`]. Must be in the range
    /// `0..4`.
    ///
    /// There are a few points that should be kept in mind when specifying this
    /// option:
    ///
    ///  - It's [`EntryPoint`][]'s caller that is responsible for ensuring the
    ///    specified privilege level is entered. Calling the entry points from
    ///    other privilege levels will cause an undefined behavior.
    ///
    ///  - The current version of `riscv-rt` can only start in M-mode.
    ///    Consequently, [`use_rt!`][] is incompatible with other modes.
    ///
    ///  - The current version of [`riscv`][] provides wrapper functions which
    ///    are hard-coded to use M-mode-only CSRs, such as `mstatus.MIE`.
    ///    They don't work in lower privilege levels. You must use [CPU Lock][]
    ///    or the correct CSR (e.g., [`riscv::register::sstatus`]) the
    ///    specified privilege level directly.
    ///
    /// [`EntryPoint`]: crate::EntryPoint
    /// [CPU Lock]: r3_core#system-states
    const PRIVILEGE_LEVEL: u8 = PRIVILEGE_LEVEL_MACHINE;
}

/// The RISC-V privilege level encoding for the machine level.
pub const PRIVILEGE_LEVEL_MACHINE: u8 = 0b11;
/// The RISC-V privilege level encoding for the supervisor level.
pub const PRIVILEGE_LEVEL_SUPERVISOR: u8 = 0b01;
/// The RISC-V privilege level encoding for the user/application level.
pub const PRIVILEGE_LEVEL_USER: u8 = 0b00;

/// Define a kernel trait type implementing [`PortThreading`],
/// [`PortInterrupts`], [`InterruptControllerToPort`], and [`EntryPoint`].
/// **Requires [`ThreadingOptions`], [`Timer`], and [`InterruptController`].**
///
/// [`PortThreading`]: r3_kernel::PortThreading
/// [`PortInterrupts`]: r3_kernel::PortInterrupts
/// [`EntryPoint`]: crate::EntryPoint
/// [`InterruptControllerToPort`]: crate::InterruptControllerToPort
/// [`InterruptController`]: crate::InterruptController
/// [`Timer`]: crate::Timer
#[macro_export]
macro_rules! use_port {
    (unsafe $vis:vis struct $Traits:ident) => {
        $vis struct $Traits;

        mod port_riscv_impl {
            use super::$Traits;
            use $crate::r3_core::kernel::{
                ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
                PendInterruptLineError, QueryInterruptLineError, SetInterruptLinePriorityError,
                interrupt::InterruptHandlerFn,
            };
            use $crate::r3_kernel::{
                TaskCb, PortToKernel, PortInterrupts, Port, PortThreading, UTicks, PortTimer,
                KernelCfg2,
            };
            use $crate::core::ops::Range;
            use $crate::{
                threading::imp::{State, TaskState, PortInstance, CsrSet, NumTy},
                ThreadingOptions, EntryPoint, InterruptController,
                InterruptControllerToPort,
            };

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

                type Csr = CsrSet<$Traits>;
                type Priv = NumTy<{ <$Traits as ThreadingOptions>::PRIVILEGE_LEVEL as usize }>;
            }

            impl EntryPoint for $Traits {
                unsafe fn start() -> ! {
                    unsafe { PORT_STATE.port_boot::<Self>() };
                }

                const TRAP_HANDLER: unsafe extern "C" fn() -> ! = State::exception_handler::<Self>;
            }

            impl InterruptControllerToPort for $Traits {
                unsafe fn enable_external_interrupts() {
                    unsafe { PORT_STATE.enable_external_interrupts::<Self>() }
                }

                unsafe fn disable_external_interrupts() {
                    unsafe { PORT_STATE.disable_external_interrupts::<Self>() }
                }
            }

            // Assume `$Traits: KernelTraits`
            unsafe impl PortThreading for $Traits {
                type PortTaskState = TaskState;
                #[allow(clippy::declare_interior_mutable_const)]
                const PORT_TASK_STATE_INIT: Self::PortTaskState =
                    $crate::r3_core::utils::Init::INIT;

                // The minimum stack size for all tests to pass. I found debug
                // formatting to be particularly memory-hungry.
                const STACK_DEFAULT_SIZE: usize = 512 * $crate::core::mem::size_of::<usize>();

                // RISC-V ELF psABI: "[...] the stack pointer shall be aligned
                // to a 128-bit boundary upon procedure entry."
                //
                // FIXME: This can be relaxed for the ILP32E calling convention
                // (applicable to RV32E), where `sp` is only required to be
                // aligned to a word boundary.
                const STACK_ALIGN: usize = 16;

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
