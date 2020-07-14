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

use constance::{
    kernel::{
        ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
        PendInterruptLineError, Port, PortToKernel, QueryInterruptLineError,
        SetInterruptLinePriorityError, TaskCb, UTicks,
    },
    prelude::*,
    utils::{intrusive_list::StaticListHead, Init},
};
use core::{cell::UnsafeCell, mem::MaybeUninit, ops::Range, slice};

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

/// Implemented on a system type by [`use_port!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_port!`].
#[doc(hidden)]
pub unsafe trait PortInstance: Kernel + Port<PortTaskState = TaskState> + PortCfg {
    fn port_state() -> &'static State;
}

/// The configuration of the port.
///
/// # Safety
///
///  - `interrupt_stack_top` must return a valid stack pointer. The default
///    implementation evaluates `*(SCB.VTOR as *const u32)`, which should
///    produce a valid value (this is usually true).
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
        // Safety: We claimed the ownership of `Peripherals`
        let peripherals = unsafe { cortex_m::Peripherals::steal() };

        // Safety: `unsafe trait`
        unsafe { (peripherals.SCB.vtor.read() as *const usize).read_volatile() }
    }
}

/// Converts [`InterruptNum`] to [`cortex_m::interrupt::Nr`].
struct Int(InterruptNum);

#[cfg(target_os = "none")]
unsafe impl cortex_m::interrupt::Nr for Int {
    #[inline]
    fn nr(&self) -> u8 {
        (self.0 - INTERRUPT_EXTERNAL0) as _
    }
}

#[doc(hidden)]
pub struct State {}

#[doc(hidden)]
#[derive(Debug)]
#[repr(C)]
pub struct TaskState {
    sp: UnsafeCell<u32>,
}

unsafe impl Sync for TaskState {}

impl State {
    pub const fn new() -> Self {
        Self {}
    }
}

impl Init for TaskState {
    const INIT: Self = Self {
        sp: UnsafeCell::new(0),
    };
}

impl State {
    pub unsafe fn port_boot<System: PortInstance>(&self) -> ! {
        unsafe { self.enter_cpu_lock::<System>() };

        // Claim the ownership of `Peripherals`
        let mut peripherals = cortex_m::Peripherals::take().unwrap();

        // Set the priorities of SVCall and PendSV
        // Safety: We don't make "priority-based critical sections"
        unsafe {
            peripherals
                .SCB
                .set_priority(cortex_m::peripheral::scb::SystemHandler::SVCall, 0xff);
            peripherals
                .SCB
                .set_priority(cortex_m::peripheral::scb::SystemHandler::PendSV, 0xff);
        }

        // Safety: We are a port, so it's okay to call this
        unsafe {
            <System as PortToKernel>::boot();
        }
    }

    pub unsafe fn dispatch_first_task<System: PortInstance>(&'static self) -> ! {
        // Find the top of the stack
        // Safety: Only `dispatch_first_task` can call this method
        let msp_top = unsafe { System::interrupt_stack_top() };

        // Re-enable interrupts so `svc` can execute
        unsafe { self.leave_cpu_lock::<System>() };

        // TODO: PendSV taking in this period can be a problem
        //
        // Consider the following execution sequence:
        //
        //  1. `running_task = task1`
        //  2. The call to `leave_cpu_lock` above
        //  3. An interrupt handler starts running
        //  4. The interrupt handler pends PendSV
        //  5. PendSV saves `PSP` to `task1`, but `PSP` doesn't actually
        //     represent `task1`'s stack pointer at this point

        llvm_asm!("
            # Reset MSP to the top of the stack, effectively discarding the
            # current context
            msr msp, $0

            # TODO: Set MSPLIM on Armv8-M

            # Transfer the control to `handle_sv_call`, which will dispatch the
            # first task and will never return to this thread
            svc 42
        "::"r"(msp_top)::"volatile");

        unreachable!()
    }

    pub unsafe fn yield_cpu<System: PortInstance>(&'static self) {
        // Safety: See `use_port!`
        cortex_m::peripheral::SCB::set_pendsv();
    }

    pub unsafe fn exit_and_dispatch<System: PortInstance>(
        &'static self,
        task: &'static TaskCb<System>,
    ) -> ! {
        unsafe { self.leave_cpu_lock::<System>() };

        // TODO: PendSV taking in this period can be a problem
        //
        // Consider the following execution sequence:
        //
        //  1. The call to `leave_cpu_lock` above (task A)
        //  2. An interrupt handler starts running
        //  3. The interrupt handler activates task A, which initializes the
        //     context of task A
        //  4. The interrupt handler returns, but the corresponding exception
        //     frame is gone, causing an unpredictable behavior

        llvm_asm!("svc 42"::::"volatile");
        unreachable!()
    }

    #[inline(always)]
    pub unsafe fn handle_pend_sv<System: PortInstance>(&'static self)
    where
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: core::borrow::BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        // Precondition:
        //  - `EXC_RETURN.Mode == 1` - Exception was taken in Thread mode. This
        //    is true because PendSV is configured with the lowest priority.
        //  - `SPSEL.Mode == 1` - The exception frame was stacked to PSP.

        // Compilation assumption:
        //  - The compiled code does not use any registers other than r0-r3
        //    before entering the inline assembly code below.
        //  - This is the top-level function in the PendSV handler. That is,
        //    the compiler must really inline `handle_pend_sv`.

        let running_task_ref = unsafe { System::state().running_task_ref() };

        extern "C" fn choose_next_task<System: PortInstance>() {
            // Choose the next task to run
            unsafe { State::enter_cpu_lock_inner::<System>() };

            // Safety: CPU Lock active
            unsafe { System::choose_running_task() };

            unsafe { State::leave_cpu_lock_inner::<System>() };
        }

        llvm_asm!("
            # Save the context of the previous task
            #
            #    [r0 = &running_task, r4-r11 = context, lr = EXC_RETURN]
            #
            #    r1 = running_task
            #    r2 = psp as *u32 - 10
            #
            #    r2[0..8] = {r4-r11}
            #    r2[8] = lr (EXC_RETURN)
            #    r2[9] = control
            #    r1.port_task_state.sp = r2
            #
            #    [r0 = &running_task]

            ldr r1, [r0]
            mrs r2, psp
            mrs r3, control
            sub r2, #40
            str r2, [r1]
            stm r2, {r4-r11}
            str lr, [r2, 32]
            str r3, [r2, 36]

            # Choose the next task to run
        .LChooseTask:
            push {r0}
            bl $1
            pop {r0}

            # Restore the context of the next task
            # TODO: Handle the case where `running_task.is_none()` better
            #        - Should use the `wfi` instruction
            #        - Should wait in Thread mode
            #
            #    [r0 = &running_task]
            #
            #    r1 = running_task
            #    if r1.is_none() { goto LChooseTask; }
            #    r2 = r1.port_task_state.sp
            #
            #    {r4-r11} = r2[0..8]
            #    lr = r2[8]
            #    control = r2[9]
            #    psp = &r2[10]
            #
            #    [r4-r11 = context, lr = EXC_RETURN]

            ldr r1, [r0]
            cbz r1, LChooseTask
            ldr r2, [r1]
            ldr lr, [r2, 32]
            ldr r3, [r2, 36]
            msr control, r3
            ldmia r2, {r4-r11}
            add r2, #40
            msr psp, r2
        "
        :
        :   "{r0}"(running_task_ref),
            "X"(choose_next_task::<System> as extern fn())
        :
        :   "volatile");
    }

    #[inline(always)]
    pub unsafe fn handle_sv_call<System: PortInstance>(&'static self)
    where
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: core::borrow::BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        // TODO: Check SVC code?

        // Precondition:
        //  - `EXC_RETURN.Mode == 1` - Exception was taken in Thread mode. This
        //    is true because SVCall is configured with the lowest priority.

        // Compilation assumption:
        //  - The compiled code does not use any registers other than r0-r3
        //    before entering the inline assembly code below.
        //  - This is the top-level function in the SVCall handler. That is,
        //    the compiler must really inline `handle_sv_call`.

        let running_task_ref = unsafe { System::state().running_task_ref() };

        extern "C" fn choose_next_task<System: PortInstance>() {
            // Choose the next task to run
            unsafe { State::enter_cpu_lock_inner::<System>() };

            // Safety: CPU Lock active
            unsafe { System::choose_running_task() };

            unsafe { State::leave_cpu_lock_inner::<System>() };
        }

        llvm_asm!("
            # Choose the next task to run
            # TODO: This is redundant for `dispatch_first_task`
        .LChooseTask2:
            push {r0}
            bl $1
            pop {r0}

            # Restore the context of the next task.
            # TODO: Handle the case where `running_task.is_none()` better
            #
            #    [r0 = &running_task]
            #
            #    r1 = running_task
            #    if r1.is_none() { goto LChooseTask2; }
            #    r2 = r1.port_task_state.sp
            #
            #    {r4-r11} = r2[0..8]
            #    lr = r2[8]
            #    control = r2[9]
            #    psp = &r2[10]
            #
            #    [r4-r11 = context, lr = EXC_RETURN]

            ldr r1, [r0]
            cbz r1, LChooseTask2
            ldr r2, [r1]
            ldr lr, [r2, 32]
            ldr r3, [r2, 36]
            msr control, r3
            ldmia r2, {r4-r11}
            add r2, r2, #40
            msr psp, r2
        "
        :
        :   "{r0}"(running_task_ref)
            "X"(choose_next_task::<System> as extern fn())
        :
        :   "volatile");
    }

    #[inline(always)]
    pub unsafe fn enter_cpu_lock<System: PortInstance>(&self) {
        unsafe { Self::enter_cpu_lock_inner::<System>() };
    }

    #[inline(always)]
    unsafe fn enter_cpu_lock_inner<System: PortInstance>() {
        if System::CPU_LOCK_PRIORITY_MASK > 0 {
            // Set `BASEPRI` to `CPU_LOCK_PRIORITY_MASK`
            unsafe { cortex_m::register::basepri::write(System::CPU_LOCK_PRIORITY_MASK) };
        } else {
            // Set `PRIMASK` to `1`
            cortex_m::interrupt::disable();
        }
    }

    #[inline(always)]
    pub unsafe fn leave_cpu_lock<System: PortInstance>(&'static self) {
        unsafe { Self::leave_cpu_lock_inner::<System>() };
    }

    #[inline(always)]
    unsafe fn leave_cpu_lock_inner<System: PortInstance>() {
        if System::CPU_LOCK_PRIORITY_MASK > 0 {
            // Set `BASEPRI` to `0` (no masking)
            unsafe { cortex_m::register::basepri::write(0) };
        } else {
            // Set `PRIMASK` to `0`
            unsafe { cortex_m::interrupt::enable() };
        }
    }

    pub unsafe fn initialize_task_state<System: PortInstance>(
        &self,
        task: &'static TaskCb<System>,
    ) {
        let stack = task.attr.stack.as_ptr();
        let mut sp = (stack as *mut u8).wrapping_add(stack.len()) as *mut MaybeUninit<u32>;
        // TODO: Enforce minimum stack size

        // Exception frame (automatically saved and restored as part of
        // the architectually-defined exception entry/return sequence)
        let exc_frame = unsafe {
            sp = sp.wrapping_sub(8);
            slice::from_raw_parts_mut(sp, 8)
        };

        // R0: Parameter to the entry point
        exc_frame[0] = MaybeUninit::new(task.attr.entry_param as u32);
        // R1-R3, R12: Uninitialized
        exc_frame[1] = MaybeUninit::new(0x01010101);
        exc_frame[2] = MaybeUninit::new(0x02020202);
        exc_frame[3] = MaybeUninit::new(0x03030303);
        exc_frame[4] = MaybeUninit::new(0x12121212);
        // LR: The return address
        exc_frame[5] = MaybeUninit::new(System::exit_task as usize as u32);
        // PC: The entry point
        exc_frame[6] = MaybeUninit::new(task.attr.entry_point as usize as u32);
        // xPSR
        exc_frame[7] = MaybeUninit::new(0x01000000);

        // Extra context (saved and restored by our code as part of context
        // switching)
        let extra_ctx = unsafe {
            sp = sp.wrapping_sub(10);
            slice::from_raw_parts_mut(sp, 10)
        };

        // R4-R11: Uninitialized
        extra_ctx[0] = MaybeUninit::new(0x04040404);
        extra_ctx[1] = MaybeUninit::new(0x05050505);
        extra_ctx[2] = MaybeUninit::new(0x06060606);
        extra_ctx[3] = MaybeUninit::new(0x07070707);
        extra_ctx[4] = MaybeUninit::new(0x08080808);
        extra_ctx[5] = MaybeUninit::new(0x09090909);
        extra_ctx[6] = MaybeUninit::new(0x10101010);
        extra_ctx[7] = MaybeUninit::new(0x11111111);
        // EXC_RETURN: 0xfffffffd (“Return to Thread Mode; Exception return gets
        //             state from the Process stack; On return execution uses
        //             the Process Stack.”)
        // TODO: This differs for Armv8-M
        // TODO: Plus, we shouldn't hard-code this here
        extra_ctx[8] = MaybeUninit::new(0xfffffffd);
        // CONTROL: SPSEL = 1 (Use PSP)
        extra_ctx[9] = MaybeUninit::new(0x00000002);
        // TODO: Secure context (Armv8-M)
        // TODO: Floating point registers
        // TODO: PSPLIM

        let task_state = &task.port_task_state;
        unsafe { *task_state.sp.get() = sp as _ };
    }

    #[inline(always)]
    pub fn is_cpu_lock_active<System: PortInstance>(&self) -> bool {
        if System::CPU_LOCK_PRIORITY_MASK > 0 {
            cortex_m::register::basepri::read() != 0
        } else {
            cortex_m::register::primask::read().is_inactive()
        }
    }

    pub fn is_task_context<System: PortInstance>(&self) -> bool {
        cortex_m::register::control::read().spsel() == cortex_m::register::control::Spsel::Psp
    }

    pub fn set_interrupt_line_priority<System: PortInstance>(
        &'static self,
        num: InterruptNum,
        priority: InterruptPriority,
    ) -> Result<(), SetInterruptLinePriorityError> {
        // Safety: We claimed the ownership of `Peripherals`
        let mut peripherals = unsafe { cortex_m::Peripherals::steal() };

        if !INTERRUPT_PRIORITY_RANGE.contains(&priority) || !INTERRUPT_NUM_RANGE.contains(&num) {
            Err(SetInterruptLinePriorityError::BadParam)
        } else if num >= INTERRUPT_EXTERNAL0 {
            // Safety: We don't make "priority-based critical sections"
            unsafe { peripherals.NVIC.set_priority(Int(num), priority as _) };
            Ok(())
        } else if num == INTERRUPT_SYSTICK {
            // Safety: We don't make "priority-based critical sections"
            unsafe {
                peripherals.SCB.set_priority(
                    cortex_m::peripheral::scb::SystemHandler::SysTick,
                    priority as _,
                )
            };
            Ok(())
        } else {
            Err(SetInterruptLinePriorityError::BadParam)
        }
    }

    #[inline]
    pub fn enable_interrupt_line<System: PortInstance>(
        &'static self,
        num: InterruptNum,
    ) -> Result<(), EnableInterruptLineError> {
        // Safety: We claimed the ownership of `Peripherals`
        let mut peripherals = unsafe { cortex_m::Peripherals::steal() };

        if !INTERRUPT_NUM_RANGE.contains(&num) {
            Err(EnableInterruptLineError::BadParam)
        } else if num >= INTERRUPT_EXTERNAL0 {
            // Safety: We don't make "mask-based critical sections"
            unsafe { cortex_m::peripheral::NVIC::unmask(Int(num)) };
            Ok(())
        } else {
            Err(EnableInterruptLineError::BadParam)
        }
    }

    #[inline]
    pub fn disable_interrupt_line<System: PortInstance>(
        &self,
        num: InterruptNum,
    ) -> Result<(), EnableInterruptLineError> {
        // Safety: We claimed the ownership of `Peripherals`
        let mut peripherals = unsafe { cortex_m::Peripherals::steal() };

        if !INTERRUPT_NUM_RANGE.contains(&num) {
            Err(EnableInterruptLineError::BadParam)
        } else if num >= INTERRUPT_EXTERNAL0 {
            cortex_m::peripheral::NVIC::mask(Int(num));
            Ok(())
        } else {
            Err(EnableInterruptLineError::BadParam)
        }
    }

    #[inline]
    pub fn pend_interrupt_line<System: PortInstance>(
        &'static self,
        num: InterruptNum,
    ) -> Result<(), PendInterruptLineError> {
        if !INTERRUPT_NUM_RANGE.contains(&num) {
            Err(PendInterruptLineError::BadParam)
        } else if num >= INTERRUPT_EXTERNAL0 {
            cortex_m::peripheral::NVIC::pend(Int(num));
            Ok(())
        } else if num == INTERRUPT_SYSTICK {
            cortex_m::peripheral::SCB::set_pendst();
            Ok(())
        } else {
            Err(PendInterruptLineError::BadParam)
        }
    }

    #[inline]
    pub fn clear_interrupt_line<System: PortInstance>(
        &self,
        num: InterruptNum,
    ) -> Result<(), ClearInterruptLineError> {
        if !INTERRUPT_NUM_RANGE.contains(&num) {
            Err(ClearInterruptLineError::BadParam)
        } else if num >= INTERRUPT_EXTERNAL0 {
            cortex_m::peripheral::NVIC::unpend(Int(num));
            Ok(())
        } else if num == INTERRUPT_SYSTICK {
            cortex_m::peripheral::SCB::clear_pendst();
            Ok(())
        } else {
            Err(ClearInterruptLineError::BadParam)
        }
    }

    #[inline]
    pub fn is_interrupt_line_pending<System: PortInstance>(
        &self,
        num: InterruptNum,
    ) -> Result<bool, QueryInterruptLineError> {
        if !INTERRUPT_NUM_RANGE.contains(&num) {
            Err(QueryInterruptLineError::BadParam)
        } else if num >= INTERRUPT_EXTERNAL0 {
            Ok(cortex_m::peripheral::NVIC::is_pending(Int(num)))
        } else if num == INTERRUPT_SYSTICK {
            Ok(cortex_m::peripheral::SCB::is_pendst_pending())
        } else {
            Err(QueryInterruptLineError::BadParam)
        }
    }

    #[inline(always)]
    pub unsafe fn handle_sys_tick<System: PortInstance>(&'static self)
    where
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: core::borrow::BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        if let Some(x) = System::INTERRUPT_HANDLERS.get(INTERRUPT_SYSTICK) {
            // Safety: It's a first-level interrupt handler here. CPU Lock inactive
            unsafe { x() };
        }
    }

    pub const MAX_TICK_COUNT: UTicks = UTicks::MAX;
    pub const MAX_TIMEOUT: UTicks = UTicks::MAX / 2;
    pub fn tick_count<System: PortInstance>(&self) -> UTicks {
        0 // TODO
    }

    pub fn pend_tick_after<System: PortInstance>(&self, tick_count_delta: UTicks) {
        if tick_count_delta < Self::MAX_TIMEOUT {
            todo!("pend_tick_after")
        }
        // TODO
    }

    pub fn pend_tick<System: PortInstance>(&'static self) {
        todo!("pend_tick")
        // TODO
    }
}

/// Used by `use_port!`
#[doc(hidden)]
#[derive(Clone, Copy)]
pub union InterruptHandler {
    undefined: usize,
    defined: constance::kernel::cfg::InterruptHandlerFn,
}

#[doc(hidden)]
pub type InterruptHandlerTable = [InterruptHandler; 240];

/// Used by `use_port!`
#[doc(hidden)]
pub const fn make_interrupt_handler_table<System: PortInstance>() -> InterruptHandlerTable
where
    // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
    System::TaskReadyQueue: core::borrow::BorrowMut<[StaticListHead<TaskCb<System>>]>,
{
    let mut table = [InterruptHandler { undefined: 0 }; 240];
    let mut i = 0;

    // FIXME: Work-around for `for` being unsupported in `const fn`
    while i < table.len() {
        table[i] = if let Some(x) = System::INTERRUPT_HANDLERS.get(i + 16) {
            InterruptHandler { defined: x }
        } else {
            InterruptHandler { undefined: 0 }
        };
        i += 1;
    }

    // Disallow registering in range `0..16` except for SysTick
    i = 0;
    // FIXME: Work-around for `for` being unsupported in `const fn`
    while i < 16 {
        if i != INTERRUPT_SYSTICK {
            // FIXME: `Option::is_some` is not `const fn` yet
            // TODO: This check trips even if no handler is registered at `i`
            #[cfg(any())]
            if let Some(_) = System::INTERRUPT_HANDLERS.get(i) {
                panic!(
                    "registering a handler for a non-internal exception is \
                    disallowed except for SysTick"
                );
            }
        }
        i += 1;
    }

    table
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
            use $crate::{State, TaskState, PortInstance, PortCfg};

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
        static __INTERRUPTS: $crate::InterruptHandlerTable =
            $crate::make_interrupt_handler_table::<$sys>();

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
