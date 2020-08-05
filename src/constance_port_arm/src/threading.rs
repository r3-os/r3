use constance::{
    kernel::{Port, PortToKernel, TaskCb},
    prelude::*,
    utils::{intrusive_list::StaticListHead, Init},
};
use core::{borrow::BorrowMut, cell::UnsafeCell, mem::MaybeUninit, slice};

use super::{InterruptController, ThreadingOptions};

/// Implemented on a system type by [`use_port!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_port!`].
pub unsafe trait PortInstance:
    Kernel + Port<PortTaskState = TaskState> + ThreadingOptions + InterruptController
{
    fn port_state() -> &'static State;
}

pub struct State {
    dispatch_pending: UnsafeCell<bool>,
}

unsafe impl Sync for State {}

#[derive(Debug)]
#[repr(C)]
pub struct TaskState {
    sp: UnsafeCell<u32>,
}

unsafe impl Sync for TaskState {}

impl State {
    pub const fn new() -> Self {
        Self {
            dispatch_pending: UnsafeCell::new(false),
        }
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

        // Safety: We are a port, so it's okay to call this
        unsafe {
            <System as PortToKernel>::boot();
        }
    }

    pub unsafe fn dispatch_first_task<System: PortInstance>(&'static self) -> ! {
        debug_assert!(self.is_cpu_lock_active::<System>());

        unsafe {
            llvm_asm!("
                mov r0, sp

                # Switch to System mode
                cps #0x1f

                # `Dispatch` needs stack
                mov sp, r0

                b Dispatch
                "
            :
            :
            :
            :   "volatile");
            core::hint::unreachable_unchecked();
        }
    }

    #[inline(never)] // avoid symbol collision with `YieldReturn`
    pub unsafe fn yield_cpu<System: PortInstance>(&'static self)
    where
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        if !self.is_task_context::<System>() {
            unsafe { self.dispatch_pending.get().write_volatile(true) };
            return;
        }

        unsafe {
            llvm_asm!("
                # Push the first level context state. The return address is
                # set to `YieldReturn`. The value of CPSR is captured before
                # `cpsid i` so that interrupts are re-enabled when the current
                # task regains the control.
                #
                #   sp_usr -= 8;
                #   sp_usr[0] = r0;
                #   sp_usr[1] = r1;
                #   sp_usr[4] = r12;
                #   sp_usr[5] = lr;
                #   sp_usr[6] = &YieldReturn;
                #   sp_usr[7] = CPSR;
                #
                adr r2, YieldReturn
                mrs r3, CPSR
                push {r2, r3}
                push {r12, lr}
                subs sp, #8
                push {r0, r1}

                cpsid i
                b $0

            YieldReturn:
                "
            :
            :   "X"(Self::push_second_level_state_and_dispatch::<System> as unsafe fn() -> !)
            :   "r2", "r3"
            :   "volatile"
            );
        }
    }

    /// Do the following steps:
    ///
    ///  - **Don't** push the first-level state.
    ///  - Push the second-level state.
    ///  - Store SP to the current task's `TaskState`.
    ///  - `Dispatch:`
    ///     - Call [`constance::kernel::PortToKernel::choose_running_task`].
    ///     - Restore SP from the next scheduled task's `TaskState`.
    ///  - If there's no task to schedule, branch to [`Self::idle_task`].
    ///  - Pop the second-level state of the next scheduled task.
    ///  - `PopFirstLevelState:`
    ///     - Pop the first-level state of the next scheduled task.
    ///
    /// # Safety
    ///
    ///  - The processor should be in System mode (task context).
    ///  - SP should point to the first-level state on the current task's stack.
    ///
    #[naked]
    unsafe fn push_second_level_state_and_dispatch<System: PortInstance>() -> !
    where
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        extern "C" fn choose_and_get_next_task<System: PortInstance>(
        ) -> Option<&'static TaskCb<System>>
        where
            // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
            System::TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<System>>]>,
        {
            // Safety: CPU Lock active
            unsafe { System::choose_running_task() };

            unsafe { *System::state().running_task_ptr() }
        }

        // Compilation assumption:
        //  - The compiled code does not trash any registers other than r0-r3
        //    before entering the inline assembly code below.
        let running_task_ptr = System::state().running_task_ptr();

        unsafe {
            llvm_asm!("
                # Push the second-level context state.
                push {r4-r11}

                # Store SP to `TaskState`.
                #
                #    [r0 = &running_task]
                #    r0 = running_task
                #    r0.port_task_state.sp = sp_usr
                #
                ldr r0, [r0]
                str sp, [r0]

            .global Dispatch
            Dispatch:
                # Choose the next task to run. `choose_and_get_next_task`
                # returns the new value of `running_task`.
                bl $1

                # Restore SP from `TaskState`
                #
                #    [r0 = running_task]
                #    if r0.is_none() {
                #        goto idle_task;
                #    }
                #    sp_usr = r0.port_task_state.sp
                #
                tst r0, r0
                beq $2
                ldr sp, [r0]

                # Pop the second-level context state.
                pop {r4-r11}

            .global PopFirstLevelState
            PopFirstLevelState:
                # Resume the next task by restoring the first-level state
                #
                #   [{r4-r11, sp_usr} = resumed context]
                #
                #   {r0-r3} = SP_usr[0..4];
                #   r12 = SP_usr[4];
                #   lr = SP_usr[5];
                #   pc = SP_usr[6];
                #   CPSR = SP_usr[7];
                #   SP_usr += 8;
                #
                #   [end of procedure]
                #
                pop {r0-r3, r12, lr}
                rfeia sp!
            "
            :
            :   "{r0}"(running_task_ptr)
            ,   "X"(choose_and_get_next_task::<System> as extern fn() -> _)
            ,   "X"(Self::idle_task::<System> as unsafe fn() -> !)
            :
            :   "volatile");
            core::hint::unreachable_unchecked();
        }
    }

    /// Branch to `push_second_level_state_and_dispatch` if `dispatch_pending`
    /// is set. Otherwise, branch to `PopFirstLevelState` (thus skipping the
    /// saving/restoration of second-level states).
    #[naked]
    unsafe fn push_second_level_state_and_dispatch_shortcutting<System: PortInstance>() -> !
    where
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        // Compilation assumption:
        //  - The compiled code does not trash any registers other than r0-r3
        //    before entering the inline assembly code below.
        let dispatch_pending_ptr = System::port_state().dispatch_pending.get();

        unsafe {
            llvm_asm!("
                # Read `dispatch_pending`
                ldrb r0, [r0]
                tst r0, r0
                bne NotShortcutting

                # `dispatch_pending` is clear, meaning we are returning to the
                # same task that the current exception has interrupted.
                #
                # If we are returning to the idle task, branch to `idle_task`
                # directly because `PopFirstLevelState` can't handle this case.
                tst sp, sp
                beq $2

                b PopFirstLevelState

                # `dispatch_pending` is set, meaning `yield_cpu` was called in
                # an interrupt handler, meaning we might need to return to a
                # different task. Clear `dispatch_pending` and branch to
                # `push_second_level_state_and_dispatch`.
            NotShortcutting:
                movs r0, #0
                strb r0, [r0]
                b $1
            "
            :
            :   "{r0}"(dispatch_pending_ptr)
            ,   "X"(Self::push_second_level_state_and_dispatch::<System> as unsafe fn() -> !)
            ,   "X"(Self::idle_task::<System> as unsafe fn() -> !)
            :
            :   "volatile");
            core::hint::unreachable_unchecked();
        }
    }

    /// Enters an idle loop with IRQs unmasked.
    ///
    /// # Safety
    ///
    ///  - The processor should be in System mode (task context).
    ///  - `*System::state().running_task_ptr()` should be `None`.
    ///
    #[naked]
    unsafe fn idle_task<System: PortInstance>() -> ! {
        unsafe {
            // TODO: Use WFI
            llvm_asm!("
                movs sp, #0
                cpsie i
            IdleLoop:
                b IdleLoop
            "
            :
            :
            :
            :   "volatile");
            core::hint::unreachable_unchecked();
        }
    }

    pub unsafe fn exit_and_dispatch<System: PortInstance>(
        &'static self,
        _task: &'static TaskCb<System>,
    ) -> ! {
        unsafe {
            llvm_asm!("
                cpsid i
                b Dispatch
                "
            :
            :
            :
            :   "volatile");
            core::hint::unreachable_unchecked();
        }
    }

    #[inline(always)]
    pub unsafe fn enter_cpu_lock<System: PortInstance>(&self) {
        // TODO: support unmanaged interrupts
        unsafe { llvm_asm!("cpsid i"::::"volatile") };
    }

    #[inline(always)]
    pub unsafe fn leave_cpu_lock<System: PortInstance>(&'static self) {
        unsafe { llvm_asm!("cpsie i"::::"volatile") };
    }

    pub unsafe fn initialize_task_state<System: PortInstance>(
        &self,
        task: &'static TaskCb<System>,
    ) {
        let stack = task.attr.stack.as_ptr();
        let mut sp = (stack as *mut u8).wrapping_add(stack.len()) as *mut MaybeUninit<u32>;
        // TODO: Enforce minimum stack size

        // First-level state (always saved and restored as part of our exception
        // entry/return sequence)
        let first_level = unsafe {
            sp = sp.wrapping_sub(8);
            slice::from_raw_parts_mut(sp, 8)
        };

        // R0: Parameter to the entry point
        first_level[0] = MaybeUninit::new(task.attr.entry_param as u32);
        // R1-R3, R12: Uninitialized
        first_level[1] = MaybeUninit::new(0x01010101);
        first_level[2] = MaybeUninit::new(0x02020202);
        first_level[3] = MaybeUninit::new(0x03030303);
        first_level[4] = MaybeUninit::new(0x12121212);
        // LR: The return address
        first_level[5] = MaybeUninit::new(System::exit_task as usize as u32);
        // PC: The entry point
        first_level[6] = MaybeUninit::new(task.attr.entry_point as usize as u32);
        // CPSR: System mode
        first_level[7] = MaybeUninit::new(0x0000001f);

        // Second-level state (saved and restored only when we are doing context
        // switching)
        let extra_ctx = unsafe {
            sp = sp.wrapping_sub(8);
            slice::from_raw_parts_mut(sp, 8)
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

        let task_state = &task.port_task_state;
        unsafe { *task_state.sp.get() = sp as _ };
    }

    #[inline(always)]
    pub fn is_cpu_lock_active<System: PortInstance>(&self) -> bool {
        let cpsr: u32;
        unsafe { llvm_asm!("mrs $0, cpsr":"=r"(cpsr):::"volatile") };
        (cpsr & (1 << 7)) != 0
    }

    #[inline(always)]
    pub fn is_task_context<System: PortInstance>(&self) -> bool {
        let cpsr: u32;
        unsafe { llvm_asm!("mrs $0, cpsr":"=r"(cpsr):::"volatile") };
        (cpsr & 0xf) == 0xf // System mode
    }

    /// Implements [`crate::EntryPoint::irq_entry`]
    #[inline(always)]
    pub unsafe fn irq_entry<System: PortInstance>() -> !
    where
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        unsafe {
            llvm_asm!("
                # Adjust `lr_irq` to get the preferred return address. (The
                # required adjustment is different for each exception type.)
                adds lr, #8

                # Switch back to the background mode. The background mode is
                # indicated by SPSR.M on handler entry.
                #
                #   [{r0-r12, sp_xxx, lr_xxx, SPSR} = background context,
                #    lr_irq = preferred return address]
                #
                #   sp_irq = SPSR
                #   match sp_irq.M {
                #       Supervisor => cps Supervisor,
                #       System => cps System,
                #   }
                #
                #   [{r0-r12, sp_xxx, lr_xxx, SPSR} = background context,
                #    lr_irq = preferred return address]
                #
                mrs sp, SPSR
                tst sp, #0x8
                bne BackgroundIsTask
                cps #0x1f
                b SwitchToBackgroundEnd
            BackgroundIsTask:
                cps #0x13
            SwitchToBackgroundEnd:

                # Skip saving the first-level state if the background context
                # is an idle task.
                #
                #   if sp_xxx == 0 {
                #       [&sp_xxx == &sp_usr, running_task is None]
                #       spsr_saved = 0x8
                #       goto PushFirstLevelStateEnd;
                #   }
                #   [&sp_xxx != &sp_usr || running_task is Some(_)]
                #
                tst sp, sp
                it eq
                moveq r3, #8
                beq PushFirstLevelStateEnd

                # Save the first-level state to the background context's stack
                # (sp_xxx = SP_usr or sp_svc).
                #
                #   [{r0-r12, sp_xxx, lr_xxx, SPSR} = background context,
                #    lr_irq = preferred return address]
                #
                #   sp_xxx -= 8;
                #   sp_xxx[0..4] = {r0-r3};
                #   sp_xxx[4] = r12;
                #   sp_xxx[5] = lr_xxx;
                #
                #   [r0 = sp_xxx, {r4-r11, sp_xxx, SPSR} = background context,
                #    lr_irq = preferred return address]
                #
                subs sp, #8
                push {r0-r3, r12, lr}
                mov r0, sp

                # Switch to IRQ mode. Save the return address to the background
                # context's stack.
                #
                #   sp_xxx[6] = lr_irq;
                #   sp_xxx[7] = SPSR;
                #   spsr_saved = SPSR;
                #
                #   [r3 = spsr_saved, {r4-r11, sp_xxx} = background context]
                #
                cps #0x12
                mov r2, lr
                mrs r3, SPSR
                strd r2, r3, [r0, #24]
            PushFirstLevelStateEnd:

                # Switch to Supervisor mode.
                cps #0x13

                # Save `spsr_saved`
                push {r3}

                # TODO: align stack to 8-byte address

                bl $0

                # Are we returning to a task context?
                #
                #   match spsr_saved.M {
                #       Supervisor => {}
                #       System => {
                #           goto ReturnToTask;
                #       }
                #   }
                pop {r3}
                tst r3, #0x8
                bne ReturnToTask

                # We are returning to an outer interrupt handler. Switching the
                # processor mode or finding the next task to dispatch is
                # unnecessary in this case.
                #
                #   [&sp_xxx == &sp_svc, {r4-r11, sp_xxx} = background context]
                #
                #   {r0-r3} = sp_svc[0..4];
                #   r12 = sp_svc[4];
                #   lr_svc = sp_svc[5];
                #   pc = sp_svc[6];
                #   CPSR = sp_svc[7];
                #   sp_svc += 8;
                #
                #   [end of procedure]
                #
                cpsid i
                clrex
                pop {r0-r3, r12, lr}
                rfeia sp!

            ReturnToTask:
                cpsid i
                clrex

                # Back to System mode...
                cps #0x1f

                # Return to the task context by restoring the first-level and
                # second-level state of the next task.
                b $1
                "
            :
            :   "X"(Self::handle_irq::<System> as unsafe fn())
            ,   "X"(Self::push_second_level_state_and_dispatch_shortcutting::<System> as unsafe fn() -> !)
            :
            :   "volatile"
            );
            core::hint::unreachable_unchecked();
        }
    }

    unsafe fn handle_irq<System: PortInstance>()
    where
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        if let Some(line) = System::acknowledge_interrupt() {
            // Now that we have signaled the acknowledgement of the current
            // exception, we can start accepting nested exceptions.
            unsafe { llvm_asm!("cpsie i"::::"volatile") };

            if let Some(handler) = System::INTERRUPT_HANDLERS.get(line) {
                // Safety: The first-level interrupt handler is the only code
                //         allowed to call this
                unsafe { handler() };
            }

            System::end_interrupt(line);
        }
    }
}

/// Used by `use_port!`
pub const fn validate<System: PortInstance>() {}
