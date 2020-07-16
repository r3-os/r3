use constance::{
    kernel::{
        ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
        PendInterruptLineError, Port, PortToKernel, QueryInterruptLineError,
        SetInterruptLinePriorityError, TaskCb,
    },
    prelude::*,
    utils::{intrusive_list::StaticListHead, Init},
};
use core::{cell::UnsafeCell, mem::MaybeUninit, slice};

use super::{
    PortCfg, INTERRUPT_EXTERNAL0, INTERRUPT_NUM_RANGE, INTERRUPT_PRIORITY_RANGE, INTERRUPT_SYSTICK,
};

/// Implemented on a system type by [`use_port!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_port!`].
pub unsafe trait PortInstance: Kernel + Port<PortTaskState = TaskState> + PortCfg {
    fn port_state() -> &'static State;
}
/// Converts [`InterruptNum`] to [`cortex_m::interrupt::Nr`].
struct Int(InterruptNum);

unsafe impl cortex_m::interrupt::Nr for Int {
    #[inline]
    fn nr(&self) -> u8 {
        (self.0 - INTERRUPT_EXTERNAL0) as _
    }
}

pub struct State {}

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
        // Pend PendSV
        cortex_m::peripheral::SCB::set_pendsv();

        // Discard the current context and transfer the control to the idle
        // task. We have pended PendSV, so the dispatcher will kick in as soon
        // as the idle task releases CPU Lock.
        //
        // Safety: `CONTROL.SPSEL == 0`, Thread mode (entailed by the boot
        // context), CPU Lock active
        unsafe { Self::idle_task::<System>() };
    }

    /// Reset MSP to `interrupt_stack_top()`, release CPU Lock, and start
    /// executing the idle loop.
    ///
    /// # Safety
    ///
    /// `CONTROL.SPSEL == 0`, Thread mode, CPU Lock active
    #[inline(never)]
    unsafe extern "C" fn idle_task<System: PortInstance>() -> ! {
        // Find the top of the interrupt stack
        // Safety: Only the port can call this method
        let msp_top = unsafe { System::interrupt_stack_top() };

        llvm_asm!("
            # Reset MSP to the top of the stack, effectively discarding the
            # current context. Beyond this point, this code is considered to be
            # running in the idle task.
            #
            # The idle task uses MSP as its stack.
            mov sp, $0

            # TODO: Set MSPLIM on Armv8-M

            # Release CPU Lock
            # TODO: Choose the appropriate method based on `CPU_LOCK_PRIORITY_MASK`
            mov r0, #0
            msr primask, r0
            cpsie i

        IdleLoop:
            wfi
            b IdleLoop
        "
        :
        :   "r"(msp_top)
        :
        :   "volatile");

        unsafe { core::hint::unreachable_unchecked() };
    }

    pub unsafe fn yield_cpu<System: PortInstance>(&'static self) {
        // Safety: See `use_port!`
        cortex_m::peripheral::SCB::set_pendsv();
    }

    pub unsafe fn exit_and_dispatch<System: PortInstance>(
        &'static self,
        _task: &'static TaskCb<System>,
    ) -> ! {
        // Pend PendSV
        cortex_m::peripheral::SCB::set_pendsv();

        llvm_asm!("
            # Activate the idle task's context by switching the current SP to
            # MSP.
            # `running_task` is `None` at this point, so the processor state
            # will be consistent with `running_task` after this operation.
            mrs r0, control
            bic r0, #2
            msr control, r0

            # Transfer the control to the idle task. We have pended PendSV, so
            # the dispatcher will kick in as soon as the idle task releases CPU
            # Lock.
            #
            # Safety:
            #  - `CONTROL.SPSEL == 0` (we just set it)
            #  - Thread mode (because `exit_and_dispatch` is called in a task
            #    context),
            #  - CPU Lock active (`exit_and_dispatch`'s requirement)
            b $0
        "
        :
        :   "X"(Self::idle_task::<System> as unsafe extern fn() -> !)
        :
        :   "volatile");

        unsafe { core::hint::unreachable_unchecked() };
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
        //  - `SPSEL.Mode == 1 && running_task.is_some()` - If the interrupted
        //    context is not the idle task, the exception frame should have been
        //    stacked to PSP.
        //  - `SPSEL.Mode == 0 && running_task.is_none()` - If the interrupted
        //    context is the idle task, the exception frame should have been
        //    stacked to MSP.

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
            #    if r1.is_some() {
            #        r2 = psp as *u32 - 10
            #
            #        r2[0..8] = {r4-r11}
            #        r2[8] = lr (EXC_RETURN)
            #        r2[9] = control
            #        r1.port_task_state.sp = r2
            #    }
            #
            #    [r0 = &running_task]

            ldr r1, [r0]
            cbz r1, ChooseTask
            mrs r2, psp
            mrs r3, control
            sub r2, #40
            str r2, [r1]
            stm r2, {r4-r11}
            str lr, [r2, 32]
            str r3, [r2, 36]

            # Choose the next task to run
        ChooseTask:
            push {r0}
            bl $1
            pop {r0}

            # Restore the context of the next task
            #
            #    [r0 = &running_task]
            #
            #    r1 = running_task
            #    if r1.is_some() {
            #        r2 = r1.port_task_state.sp
            #
            #        {r4-r11} = r2[0..8]
            #        lr = r2[8]
            #        control = r2[9]
            #        psp = &r2[10]
            #    } else {
            #        // The idle task only uses r0-r3, so we can skip most steps
            #        // in this case
            #        control = 2;
            #        lr = 0xfffffff9; /* “ Return to Thread Mode; Exception
            #           return gets state from the Main stack; On return
            #           execution uses the Main Stack.” */
            #    }
            #
            #    [r4-r11 = context, lr = EXC_RETURN]

            ldr r1, [r0]
            cbz r1, RestoreIdleTask
            ldr r2, [r1]
            ldr lr, [r2, 32]
            ldr r3, [r2, 36]
            msr control, r3
            ldmia r2, {r4-r11}
            add r2, #40
            msr psp, r2
            bx lr

        RestoreIdleTask:
            mov r0, #0
            mov lr, #0xfffffff9
            msr control, r0
        "
        :
        :   "{r0}"(running_task_ref)
        ,   "X"(choose_next_task::<System> as extern fn())
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
        // All tasks use PSP. The idle task is the exception, but user
        // code cannot run in the idle task, so we can ignore this.
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
}

/// Used by `use_port!`
#[derive(Clone, Copy)]
pub union InterruptHandler {
    undefined: usize,
    defined: constance::kernel::cfg::InterruptHandlerFn,
}

pub type InterruptHandlerTable = [InterruptHandler; 240];

/// Used by `use_port!`
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
