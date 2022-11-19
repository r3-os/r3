use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    slice,
    sync::atomic::{compiler_fence, Ordering},
};
use memoffset::offset_of;
use r3_core::{
    kernel::{
        traits, ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
        PendInterruptLineError, QueryInterruptLineError, SetInterruptLinePriorityError,
    },
    utils::Init,
};
use r3_kernel::{KernelTraits, Port, PortToKernel, System, TaskCb};
use r3_portkit::{pptext::pp_asm, sym::sym_static};

use crate::{
    ThreadingOptions, INTERRUPT_EXTERNAL0, INTERRUPT_NUM_RANGE, INTERRUPT_PRIORITY_RANGE,
    INTERRUPT_SYSTICK,
};

/// Implemented on a kernel trait type by [`use_port!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_port!`].
pub unsafe trait PortInstance:
    KernelTraits + Port<PortTaskState = TaskState> + ThreadingOptions
{
    sym_static!(#[sym(p_port_state)] fn port_state() -> &State);
}
/// Converts [`InterruptNum`] to [`cortex_m::interrupt::Nr`].
#[derive(Clone, Copy)]
struct Int(InterruptNum);

unsafe impl cortex_m::interrupt::Nr for Int {
    #[inline]
    fn nr(&self) -> u8 {
        (self.0 - INTERRUPT_EXTERNAL0) as _
    }
}

pub struct State {
    /// Stores the value of `Traits::state().running_task_ptr()` so that it can
    /// be accessed in naked functions. This field is actually of type
    /// `*mut Option<&'static TaskCb<Traits>>`.
    running_task_ptr: UnsafeCell<*mut ()>,
}

impl State {
    const OFFSET_RUNNING_TASK_PTR: usize = offset_of!(State, running_task_ptr);
}

unsafe impl Sync for State {}

impl Init for State {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self {
        running_task_ptr: UnsafeCell::new(core::ptr::null_mut()),
    };
}

#[derive(Debug)]
#[repr(C)]
pub struct TaskState {
    sp: UnsafeCell<u32>,
}

unsafe impl Sync for TaskState {}

impl Init for TaskState {
    #[allow(clippy::declare_interior_mutable_const)] // it's intentional
    const INIT: Self = Self {
        sp: UnsafeCell::new(0),
    };
}

impl State {
    #[inline(always)]
    pub unsafe fn port_boot<Traits: PortInstance>(&self) -> ! {
        unsafe { self.enter_cpu_lock::<Traits>() };

        // Claim the ownership of `Peripherals`
        let mut peripherals = unsafe { cortex_m::Peripherals::steal() };

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
            <Traits as PortToKernel>::boot();
        }
    }

    #[inline(always)]
    pub unsafe fn dispatch_first_task<Traits: PortInstance>(&'static self) -> ! {
        // [tag:running_task_ptr_set_in_dft]
        unsafe { *self.running_task_ptr.get() = Traits::state().running_task_ptr() as *mut () };

        // Pend PendSV
        cortex_m::peripheral::SCB::set_pendsv();

        // Discard the current context and transfer the control to the idle
        // task. We have pended PendSV, so the dispatcher will kick in as soon
        // as the idle task releases CPU Lock.
        //
        // Safety: `CONTROL.SPSEL == 0`, Thread mode (entailed by the boot
        // context), CPU Lock active
        unsafe { Self::idle_task::<Traits>() };
    }

    /// Reset MSP to `interrupt_stack_top()`, release CPU Lock, and start
    /// executing the idle loop.
    ///
    /// # Safety
    ///
    /// `CONTROL.SPSEL == 0`, Thread mode, CPU Lock active
    #[inline(never)]
    unsafe extern "C" fn idle_task<Traits: PortInstance>() -> ! {
        // Find the top of the interrupt stack
        // Safety: Only the port can call this method
        let msp_top = unsafe { Traits::interrupt_stack_top() };

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

        if Traits::USE_WFI {
            pp_asm!(
                "
            0:
                wfi
                b 0b
            ",
                options(noreturn),
            );
        } else {
            pp_asm!(
                "
            0:
                b 0b
            ",
                options(noreturn),
            );
        }
    }

    pub unsafe fn yield_cpu<Traits: PortInstance>(&'static self) {
        // Ensure preceding memory operations are visible to the PendSV handler
        compiler_fence(Ordering::Release);

        // Safety: See `use_port!`
        cortex_m::peripheral::SCB::set_pendsv();

        // Technically this DSB isn't required for correctness, but ensures
        // PendSV is taken before the next operation.
        cortex_m::asm::dsb();

        // Ensure the PendSV handler's memory operations are visible to us
        compiler_fence(Ordering::Acquire);
    }

    #[inline(always)]
    pub unsafe fn exit_and_dispatch<Traits: PortInstance>(
        &'static self,
        _task: &'static TaskCb<Traits>,
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
                ldr r0, ={idle_task}
                bx r0
        "   } else {                                                        "
                b {idle_task}
        "   },
            idle_task = sym Self::idle_task::<Traits>,
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
    pub unsafe extern "C" fn handle_pend_sv<Traits: PortInstance>() {
        // Precondition:
        //  - `EXC_RETURN.Mode == 1` - Exception was taken in Thread mode. This
        //    is true because PendSV is configured with the lowest priority.
        //  - `SPSEL.Mode == 1 && running_task.is_some()` - If the interrupted
        //    context is not the idle task, the exception frame should have been
        //    stacked to PSP.
        //  - `SPSEL.Mode == 0 && running_task.is_none()` - If the interrupted
        //    context is the idle task, the exception frame should have been
        //    stacked to MSP.

        extern "C" fn choose_next_task<Traits: PortInstance>() {
            // Choose the next task to run
            unsafe { State::enter_cpu_lock_inner::<Traits>() };

            // Safety: CPU Lock active
            unsafe { Traits::choose_running_task() };

            unsafe { State::leave_cpu_lock_inner::<Traits>() };
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

            ldr r0, ={p_port_state}_
            ldr r0, [r0]
            ldr r0, [r0, #{OFFSET_RUNNING_TASK_PTR}]

            ldr r1, [r0]                                                    "
            if cfg!(armv6m) {                                               "
                cmp r1, #0
                beq 0f
        "   } else {                                                        "
                cbz r1, 0f
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
        0:     # ChooseTask
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
            #        // `RestoreIdleTask`
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
                beq 0f
        "   } else {                                                        "
                cbz r1, 0f
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

        0:
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
            choose_next_task = sym choose_next_task::<Traits>,
            p_port_state = sym Traits::p_port_state,
            OFFSET_RUNNING_TASK_PTR = const Self::OFFSET_RUNNING_TASK_PTR,
            options(noreturn),
        );
    }

    #[inline(always)]
    pub unsafe fn enter_cpu_lock<Traits: PortInstance>(&self) {
        unsafe { Self::enter_cpu_lock_inner::<Traits>() };
    }

    #[inline(always)]
    unsafe fn enter_cpu_lock_inner<Traits: PortInstance>() {
        #[cfg(not(any(armv6m, armv8m_base)))]
        if Traits::CPU_LOCK_PRIORITY_MASK > 0 {
            // Set `BASEPRI` to `CPU_LOCK_PRIORITY_MASK`
            unsafe { cortex_m::register::basepri::write(Traits::CPU_LOCK_PRIORITY_MASK) };

            // Synchronize with the previous owner of CPU Lock.
            //
            // The semantics of `compiler_fence` we need here for this to work
            // might be more strict than what `compiler_fence`'s documentation
            // ("the compiler may be disallowed from moving reads or writes from
            // before or after the call to the other side of the call to
            // `compiler_fence`"; but it doesn't say it can't be moved past
            // `basepri::write`, which has `options(nomem)`[2] and therefore is
            // not a memory operation) and the C++ memory model ("memory
            // synchronization ordering of non-atomic and relaxed atomic
            // accesses"[3] but there's no atomic access here) say. But on the
            // other hand, there's a code comment[1] from the `cortex-m` package
            // (maintained by Rust's official embedded devices WG[4]) suggesting
            // that `compiler_fence` can in fact prevent the reordering as
            // intended. We're going to take their word for it since we are
            // using this package anyway.
            //
            // [1]: https://github.com/rust-embedded/cortex-m/blob/92552c73d3b56dc86007450633950d16ebe0e495/asm/inline.rs#L36
            // [2]: https://github.com/rust-embedded/cortex-m/blob/92552c73d3b56dc86007450633950d16ebe0e495/asm/inline.rs#L243
            // [3]: https://en.cppreference.com/w/cpp/atomic/atomic_signal_fence
            // [4]: https://github.com/rust-embedded/wg
            compiler_fence(Ordering::Acquire);
            return;
        }

        // Set `PRIMASK` to `1`
        cortex_m::interrupt::disable();
    }

    #[inline(always)]
    pub unsafe fn leave_cpu_lock<Traits: PortInstance>(&'static self) {
        unsafe { Self::leave_cpu_lock_inner::<Traits>() };
    }

    #[inline(always)]
    unsafe fn leave_cpu_lock_inner<Traits: PortInstance>() {
        #[cfg(not(any(armv6m, armv8m_base)))]
        if Traits::CPU_LOCK_PRIORITY_MASK > 0 {
            // Synchronize with the next owner of CPU Lock.
            compiler_fence(Ordering::Release);

            // Set `BASEPRI` to `0` (no masking)
            unsafe { cortex_m::register::basepri::write(0) };
            return;
        }

        // Set `PRIMASK` to `0`
        unsafe { cortex_m::interrupt::enable() };
    }

    pub unsafe fn initialize_task_state<Traits: PortInstance>(
        &self,
        task: &'static TaskCb<Traits>,
    ) {
        let stack: *mut [u8] = task.attr.stack.as_ptr();
        let mut sp = stack
            .as_mut_ptr()
            .wrapping_add(stack.len())
            .cast::<MaybeUninit<u32>>();
        // TODO: Enforce minimum stack size

        let preload_all = cfg!(feature = "preload-registers");

        // Exception frame (automatically saved and restored as part of
        // the architectually-defined exception entry/return sequence)
        let exc_frame = unsafe {
            sp = sp.wrapping_sub(8);
            slice::from_raw_parts_mut(sp, 8)
        };

        // R0: Parameter to the entry point
        exc_frame[0] = unsafe { core::mem::transmute(task.attr.entry_param) };
        // R1-R3, R12: Uninitialized
        if preload_all {
            exc_frame[1] = MaybeUninit::new(0x01010101);
            exc_frame[2] = MaybeUninit::new(0x02020202);
            exc_frame[3] = MaybeUninit::new(0x03030303);
            exc_frame[4] = MaybeUninit::new(0x12121212);
        }
        // LR: The return address
        exc_frame[5] =
            MaybeUninit::new(<System<Traits> as traits::KernelBase>::raw_exit_task as usize as u32);
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
    pub fn is_cpu_lock_active<Traits: PortInstance>(&self) -> bool {
        #[cfg(not(any(armv6m, armv8m_base)))]
        if Traits::CPU_LOCK_PRIORITY_MASK > 0 {
            return cortex_m::register::basepri::read() != 0;
        }

        cortex_m::register::primask::read().is_inactive()
    }

    pub fn is_task_context<Traits: PortInstance>(&self) -> bool {
        // All tasks use PSP. The idle task is the exception, but user
        // code cannot run in the idle task, so we can ignore this.
        cortex_m::register::control::read().spsel() == cortex_m::register::control::Spsel::Psp
    }

    #[inline]
    pub fn is_interrupt_context<Traits: PortInstance>(&self) -> bool {
        // `IPSR.Exception != 0`
        unsafe {
            let ipsr: u32;
            pp_asm!(
                "mrs {}, ipsr",
                out(reg) ipsr,
                options(nomem, preserves_flags, nostack),
            );
            (ipsr & ((1u32 << 9) - 1)) != 0
        }
    }

    #[inline]
    pub fn is_scheduler_active<Traits: PortInstance>(&self) -> bool {
        // `runnin_task_ptr` is assigned by `dispatch_first_task`
        // [ref:running_task_ptr_set_in_dft]
        unsafe { !(*self.running_task_ptr.get()).is_null() }
    }

    pub fn set_interrupt_line_priority<Traits: PortInstance>(
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
    pub fn enable_interrupt_line<Traits: PortInstance>(
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
    pub fn disable_interrupt_line<Traits: PortInstance>(
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
    pub fn pend_interrupt_line<Traits: PortInstance>(
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
    pub fn clear_interrupt_line<Traits: PortInstance>(
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
    pub fn is_interrupt_line_pending<Traits: PortInstance>(
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
}

/// Used by `use_port!`
pub const fn validate<Traits: PortInstance>() {
    #[cfg(any(armv6m, armv8m_base))]
    assert!(
        Traits::CPU_LOCK_PRIORITY_MASK == 0,
        "`CPU_LOCK_PRIORITY_MASK` must be zero because the target architecture \
         does not have a BASEPRI register"
    );
}
