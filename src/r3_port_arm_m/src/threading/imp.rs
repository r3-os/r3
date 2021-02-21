use core::{cell::UnsafeCell, mem::MaybeUninit, slice};
use memoffset::offset_of;
use r3::{
    kernel::{
        ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
        PendInterruptLineError, Port, PortToKernel, QueryInterruptLineError,
        SetInterruptLinePriorityError, TaskCb,
    },
    prelude::*,
    utils::{Init, ZeroInit},
};
use r3_portkit::{
    pptext::pp_asm,
    sym::{sym_static, SymStaticExt},
};

use crate::{
    ThreadingOptions, INTERRUPT_EXTERNAL0, INTERRUPT_NUM_RANGE, INTERRUPT_PRIORITY_RANGE,
    INTERRUPT_SYSTICK,
};

/// Implemented on a system type by [`use_port!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_port!`].
pub unsafe trait PortInstance:
    Kernel + Port<PortTaskState = TaskState> + ThreadingOptions
{
    sym_static!(static PORT_STATE: SymStatic<State> = zeroed!());

    fn port_state() -> &'static State {
        sym_static(Self::PORT_STATE).as_ref()
    }
}
/// Converts [`InterruptNum`] to [`cortex_m::interrupt::Nr`].
struct Int(InterruptNum);

unsafe impl cortex_m::interrupt::Nr for Int {
    #[inline]
    fn nr(&self) -> u8 {
        (self.0 - INTERRUPT_EXTERNAL0) as _
    }
}

pub struct State {
    /// Stores the value of `System::state().running_task_ptr()` so that it can
    /// be accessed in naked functions. This field is actually of type
    /// `*mut Option<&'static TaskCb<System>>`.
    running_task_ptr: UnsafeCell<*mut ()>,
}

impl State {
    const OFFSET_RUNNING_TASK_PTR: usize = offset_of!(State, running_task_ptr);
}

unsafe impl Sync for State {}
unsafe impl ZeroInit for State {}

#[derive(Debug)]
#[repr(C)]
pub struct TaskState {
    sp: UnsafeCell<u32>,
}

unsafe impl Sync for TaskState {}

impl Init for TaskState {
    const INIT: Self = Self {
        sp: UnsafeCell::new(0),
    };
}

impl State {
    pub unsafe fn port_boot<System: PortInstance>(&self) -> ! {
        unsafe { self.enter_cpu_lock::<System>() };

        unsafe { *self.running_task_ptr.get() = System::state().running_task_ptr() as *mut () };

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

        pp_asm!("
            # Reset MSP to the top of the stack, effectively discarding the
            # current context. Beyond this point, this code is considered to be
            # running in the idle task.
            #
            # The idle task uses MSP as its stack.
            mov sp, {msp_top}

            # TODO: Set MSPLIM on Armv8-M

            # Release CPU Lock
            # TODO: Choose the appropriate method based on `CPU_LOCK_PRIORITY_MASK` "
            if cfg!(not(any(armv6m, armv8m_base))) {                            "
                movs r0, #0
                msr basepri, r0
        "   }                                                                   "
            cpsie i
        ",
            msp_top = in(reg) msp_top,
        );

        if System::USE_WFI {
            pp_asm!(
                "
            IdleLoopWithWfi:
                wfi
                b IdleLoopWithWfi
            ",
                options(noreturn),
            );
        } else {
            pp_asm!(
                "
            IdleLoopWithoutWfi:
                b IdleLoopWithoutWfi
            ",
                options(noreturn),
            );
        }
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

        pp_asm!("
            # Activate the idle task's context by switching the current SP to
            # MSP.
            # `running_task` is `None` at this point, so the processor state
            # will be consistent with `running_task` after this operation.
            mrs r0, control
            subs r0, #2
            msr control, r0

            # Transfer the control to the idle task. We have pended PendSV, so
            # the dispatcher will kick in as soon as the idle task releases CPU
            # Lock.
            #
            # Safety:
            #  - `CONTROL.SPSEL == 0` (we just set it)
            #  - Thread mode (because `exit_and_dispatch` is called in a task
            #    context),
            #  - CPU Lock active (`exit_and_dispatch`'s requirement)        "
            if cfg!(armv6m) {                                               "
                ldr r0, IdleTaskConst
                bx r0

                .align 2
            IdleTaskConst:
                .word {idle_task}
        "   } else {                                                        "
                b {idle_task}
        "   },
            idle_task = sym Self::idle_task::<System>,
            options(noreturn),
        );
    }

    /// The PendSV handler.
    ///
    /// # Safety
    ///
    ///  - This method must be registered as a PendSV handler. The callee-saved
    ///    registers must contain the values from the background context.
    ///
    #[naked]
    pub unsafe extern "C" fn handle_pend_sv<System: PortInstance>() {
        // Precondition:
        //  - `EXC_RETURN.Mode == 1` - Exception was taken in Thread mode. This
        //    is true because PendSV is configured with the lowest priority.
        //  - `SPSEL.Mode == 1 && running_task.is_some()` - If the interrupted
        //    context is not the idle task, the exception frame should have been
        //    stacked to PSP.
        //  - `SPSEL.Mode == 0 && running_task.is_none()` - If the interrupted
        //    context is the idle task, the exception frame should have been
        //    stacked to MSP.

        extern "C" fn choose_next_task<System: PortInstance>() {
            // Choose the next task to run
            unsafe { State::enter_cpu_lock_inner::<System>() };

            // Safety: CPU Lock active
            unsafe { System::choose_running_task() };

            unsafe { State::leave_cpu_lock_inner::<System>() };
        }

        pp_asm!("
            # Save the context of the previous task
            #
            #    <r4-r11 = context,
            #     s16-s31 = context, lr = EXC_RETURN>
            #
            #    r1 = running_task
            #    if r1.is_some():
            #        let fpu_active = cfg!(has_fpu) && (lr & FType) == 0;
            #        r2 = psp as *u32 - (if fpu_active then 26 else 10)
            #        r1.port_task_state.sp = r2
            #
            #        r2[0] = lr (EXC_RETURN)
            #        r2[1] = control
            #        r2 += 2;
            #        if fpu_active:
            #            r2[0..16] = [s16-s31]
            #            r2 += 16;
            #        r2[0..8] = [r4-r11]
            #
            #    <r0 = &running_task>

            ldr r0, ={PORT_STATE}_
            ldr r0, [r0, #{OFFSET_RUNNING_TASK_PTR}]

            ldr r1, [r0]                                                    "
            if cfg!(armv6m) {                                               "
                cmp r1, #0
                beq ChooseTask
        "   } else {                                                        "
                cbz r1, ChooseTask
        "   }                                                               "
            mrs r2, psp
            mrs r3, control
            subs r2, #40                                                    "
            if cfg!(has_fpu) {                                              "
                tst lr, #0x10
                it eq
                subeq r2, #64
        "   }                                                               "
            str r2, [r1]                                                    "
            if cfg!(any(armv6m, armv8m_base)) {                             "
                mov r1, lr
                stmia r2!, {{r1, r3}}
                stmia r2!, {{r4-r7}}
                mov r4, r8
                mov r5, r9
                mov r6, r10
                mov r7, r11
                stmia r2!, {{r4-r7}}
        "   } else {                                                        "
                strd lr, r3, [r2], #8                                       "
                if cfg!(has_fpu) {                                          "
                    it eq
                    vstmiaeq r2!, {{s16-s31}}
        "       }                                                           "
                stmia r2, {{r4-r11}}
        "   }                                                               "

            # Choose the next task to run
        ChooseTask:
            mov r5, r0
            bl {choose_next_task}
            mov r0, r5

            # Restore the context of the next task
            #
            #    <r0 = &running_task>
            #
            #    r1 = running_task
            #    if r1.is_some():
            #        r2 = r1.port_task_state.sp
            #
            #        lr = r2[0]
            #        control = r2[1]
            #        r2 += 2;
            #
            #        let fpu_active = cfg!(has_fpu) && (lr & FType) == 0;
            #        if fpu_active:
            #            [s16-s31] = r2[0..16]
            #            r2 += 16;
            #
            #        [r4-r11] = r2[0..8]
            #        r2 += 8;
            #        psp = r2
            #    else:
            #        // The idle task only uses r0-r3, so we can skip most steps
            #        // in this case
            #        control = 2;
            #        lr = 0xfffffff9; /* “ Return to Thread Mode; Exception
            #           return gets state from the Main stack; On return
            #           execution uses the Main Stack.” */
            #
            #    <r4-r11 = context, s16-s31 = context, lr = EXC_RETURN>

            ldr r1, [r0]                                                    "
            if cfg!(armv6m) {                                               "
                cmp r1, #0
                beq RestoreIdleTask
        "   } else {                                                        "
                cbz r1, RestoreIdleTask
        "   }                                                               "
            ldr r2, [r1]                                                    "
            if cfg!(any(armv6m, armv8m_base)) {                             "
                ldmia r2!, {{r0, r3}}
                mov lr, r0
                ldmia r2!, {{r4-r7}}
                ldmia r2!, {{r0, r1}}
                mov r8, r0
                mov r9, r1
                ldmia r2!, {{r0, r1}}
                mov r10, r0
                mov r11, r1
        "   } else {                                                        "
                ldrd lr, r3, [r2], #8                                       "
                if cfg!(has_fpu) {                                          "
                    tst lr, #0x10
                    it eq
                    vldmiaeq r2!, {{s16-s31}}
        "       }                                                           "
                ldmia r2!, {{r4-r11}}
        "   }                                                               "
            msr control, r3
            msr psp, r2
            bx lr

        RestoreIdleTask:
            movs r0, #0                                                     "
            if cfg!(any(armv6m, armv8m_base)) {                             "
                # 0x00000006 = !0xfffffff9
                movs r1, #6
                mvns r1, r1
                mov lr, r1
        "   } else {                                                        "
                mov lr, #0xfffffff9
        "   }                                                               "
            msr control, r0
            bx lr
        ",
            choose_next_task = sym choose_next_task::<System>,
            PORT_STATE = sym System::PORT_STATE,
            OFFSET_RUNNING_TASK_PTR = const Self::OFFSET_RUNNING_TASK_PTR,
            options(noreturn),
        );
    }

    #[inline(always)]
    pub unsafe fn enter_cpu_lock<System: PortInstance>(&self) {
        unsafe { Self::enter_cpu_lock_inner::<System>() };
    }

    #[inline(always)]
    unsafe fn enter_cpu_lock_inner<System: PortInstance>() {
        #[cfg(not(any(armv6m, armv8m_base)))]
        if System::CPU_LOCK_PRIORITY_MASK > 0 {
            // Set `BASEPRI` to `CPU_LOCK_PRIORITY_MASK`
            unsafe { cortex_m::register::basepri::write(System::CPU_LOCK_PRIORITY_MASK) };
            return;
        }

        // Set `PRIMASK` to `1`
        cortex_m::interrupt::disable();
    }

    #[inline(always)]
    pub unsafe fn leave_cpu_lock<System: PortInstance>(&'static self) {
        unsafe { Self::leave_cpu_lock_inner::<System>() };
    }

    #[inline(always)]
    unsafe fn leave_cpu_lock_inner<System: PortInstance>() {
        #[cfg(not(any(armv6m, armv8m_base)))]
        if System::CPU_LOCK_PRIORITY_MASK > 0 {
            // Set `BASEPRI` to `0` (no masking)
            unsafe { cortex_m::register::basepri::write(0) };
            return;
        }

        // Set `PRIMASK` to `0`
        unsafe { cortex_m::interrupt::enable() };
    }

    pub unsafe fn initialize_task_state<System: PortInstance>(
        &self,
        task: &'static TaskCb<System>,
    ) {
        let stack = task.attr.stack.as_ptr();
        let mut sp = (stack as *mut u8).wrapping_add(stack.len()) as *mut MaybeUninit<u32>;
        // TODO: Enforce minimum stack size

        let preload_all = cfg!(feature = "preload-registers");

        // Exception frame (automatically saved and restored as part of
        // the architectually-defined exception entry/return sequence)
        let exc_frame = unsafe {
            sp = sp.wrapping_sub(8);
            slice::from_raw_parts_mut(sp, 8)
        };

        // R0: Parameter to the entry point
        exc_frame[0] = MaybeUninit::new(task.attr.entry_param as u32);
        // R1-R3, R12: Uninitialized
        if preload_all {
            exc_frame[1] = MaybeUninit::new(0x01010101);
            exc_frame[2] = MaybeUninit::new(0x02020202);
            exc_frame[3] = MaybeUninit::new(0x03030303);
            exc_frame[4] = MaybeUninit::new(0x12121212);
        }
        // LR: The return address
        exc_frame[5] = MaybeUninit::new(System::exit_task as usize as u32);
        // PC: The entry point - The given function pointer has its LSB set to
        // signify that the target is a Thumb function (that's the only valid
        // mode on Arm-M) as required by the BLX instruction. In an exception
        // frame, however, the bit should be cleared to represent the exact
        // program counter value.
        // (Until Armv7-M) “UNPREDICTABLE if the new PC not halfword aligned”
        // (Since Armv8-M) “Bit[0] of the ReturnAddress is discarded”
        exc_frame[6] = MaybeUninit::new(task.attr.entry_point as usize as u32 & !1);
        // xPSR
        exc_frame[7] = MaybeUninit::new(0x01000000);

        // Extra context (saved and restored by our code as part of context
        // switching)
        let extra_ctx = unsafe {
            sp = sp.wrapping_sub(10);
            slice::from_raw_parts_mut(sp, 10)
        };

        // EXC_RETURN: 0xfffffffd (“Return to Thread Mode; Exception return gets
        //             state from the Process stack; On return execution uses
        //             the Process Stack.”)
        // TODO: This differs for Armv8-M
        // TODO: Plus, we shouldn't hard-code this here
        extra_ctx[0] = MaybeUninit::new(0xfffffffd);
        // CONTROL: SPSEL = 1 (Use PSP)
        extra_ctx[1] = MaybeUninit::new(0x00000002);
        // TODO: Secure context (Armv8-M)
        // TODO: PSPLIM

        // R4-R11: Uninitialized
        if preload_all {
            extra_ctx[2] = MaybeUninit::new(0x04040404);
            extra_ctx[3] = MaybeUninit::new(0x05050505);
            extra_ctx[4] = MaybeUninit::new(0x06060606);
            extra_ctx[5] = MaybeUninit::new(0x07070707);
            extra_ctx[6] = MaybeUninit::new(0x08080808);
            extra_ctx[7] = MaybeUninit::new(0x09090909);
            extra_ctx[8] = MaybeUninit::new(0x10101010);
            extra_ctx[9] = MaybeUninit::new(0x11111111);
        }

        let task_state = &task.port_task_state;
        unsafe { *task_state.sp.get() = sp as _ };
    }

    #[inline(always)]
    pub fn is_cpu_lock_active<System: PortInstance>(&self) -> bool {
        #[cfg(not(any(armv6m, armv8m_base)))]
        if System::CPU_LOCK_PRIORITY_MASK > 0 {
            return cortex_m::register::basepri::read() != 0;
        }

        cortex_m::register::primask::read().is_inactive()
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
    pub unsafe fn handle_sys_tick<System: PortInstance>(&'static self) {
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
    defined: r3::kernel::cfg::InterruptHandlerFn,
}

const NUM_INTERRUPTS: usize = if cfg!(armv6m) { 32 } else { 240 };

pub type InterruptHandlerTable = [InterruptHandler; NUM_INTERRUPTS];

/// Used by `use_port!`
pub const fn make_interrupt_handler_table<System: PortInstance>() -> InterruptHandlerTable {
    let mut table = [InterruptHandler { undefined: 0 }; NUM_INTERRUPTS];
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
            // TODO: This check trips even if no handler is registered at `i`
            #[cfg(any())]
            assert!(
                System::INTERRUPT_HANDLERS.get(i).is_none(),
                "registering a handler for a non-internal exception is \
                disallowed except for SysTick"
            );
        }
        i += 1;
    }

    table
}

/// Used by `use_port!`
pub const fn validate<System: PortInstance>() {
    #[cfg(any(armv6m, armv8m_base))]
    assert!(
        System::CPU_LOCK_PRIORITY_MASK == 0,
        "`CPU_LOCK_PRIORITY_MASK` must be zero because the target architecture \
         does not have a BASEPRI register"
    );
}

/// Define a `PendSV` symbol at the PendSV handler implementation.
///
/// Just including this function in linking causes the intended effect. Calling
/// this function at runtime will have no effect.
#[naked]
pub extern "C" fn register_pend_sv_in_rt<System: PortInstance>() {
    // `global_asm!` can't refer to mangled symbols, so we need to use `asm!`
    // to do this.
    unsafe {
        asm!("
            bx lr

            .global PendSV
            PendSV = {}
        ",
            sym State::handle_pend_sv::<System>,
            options(noreturn),
        );
    }
}
