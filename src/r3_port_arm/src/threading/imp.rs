use core::{arch::asm, cell::UnsafeCell, mem::MaybeUninit, slice};
use memoffset::offset_of;
use r3_core::{kernel::traits, utils::Init};
use r3_kernel::{KernelTraits, Port, PortToKernel, System, TaskCb};
use r3_portkit::sym::sym_static;

use super::cfg::{InterruptController, ThreadingOptions, Timer};

/// Implemented on a kernel trait type by [`use_port!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_port!`].
pub unsafe trait PortInstance:
    KernelTraits + Port<PortTaskState = TaskState> + ThreadingOptions + InterruptController + Timer
{
    sym_static!(#[sym(p_port_state)] fn port_state() -> &State);
}

#[repr(C)]
pub struct State {
    dispatch_pending: UnsafeCell<bool>,
    main_stack: UnsafeCell<usize>,
    /// Stores the value of `System::state().running_task_ptr()` so that it can
    /// be accessed in naked functions. This field is actually of type
    /// `*mut Option<&'static TaskCb<System>>`.
    running_task_ptr: UnsafeCell<*mut ()>,
}

impl State {
    const OFFSET_DISPATCH_PENDING: usize = offset_of!(State, dispatch_pending);
    const OFFSET_MAIN_STACK: usize = offset_of!(State, main_stack);
    const OFFSET_RUNNING_TASK_PTR: usize = offset_of!(State, running_task_ptr);
}

unsafe impl Sync for State {}

impl Init for State {
    const INIT: Self = Self {
        dispatch_pending: UnsafeCell::new(false),
        main_stack: UnsafeCell::new(0),
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

        unsafe { *self.running_task_ptr.get() = Traits::state().running_task_ptr() as *mut () };

        // Safety: We are the port, so it's okay to call this
        unsafe { <Traits as InterruptController>::init() };

        // Safety: We are the port, so it's okay to call this
        unsafe { <Traits as Timer>::init() };

        // Safety: We are the port, so it's okay to call this
        unsafe { <Traits as PortToKernel>::boot() };
    }

    #[inline(always)]
    pub unsafe fn dispatch_first_task<Traits: PortInstance>(&'static self) -> ! {
        debug_assert!(self.is_cpu_lock_active::<Traits>());

        unsafe {
            asm!("
                mov r0, sp

                # Switch to System mode
                cps #0x1f

                # `dispatch` needs stack
                mov sp, r0

                # Save the stack pointer for later use
                # [tag:arm_main_stack_assigned_in_dft]
                str r0, [r1]

                b {push_second_level_state_and_dispatch}.dispatch
                ",
                push_second_level_state_and_dispatch =
                    sym Self::push_second_level_state_and_dispatch::<Traits>,
                in("r1") self.main_stack.get(),
                options(noreturn),
            );
        }
    }

    #[inline(never)] // avoid symbol collision with `YieldReturn`
    pub unsafe fn yield_cpu<Traits: PortInstance>(&'static self) {
        if !self.is_task_context::<Traits>() {
            unsafe { self.dispatch_pending.get().write_volatile(true) };
            return;
        }

        unsafe {
            asm!("
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
                adr r2, 0f
                mrs r3, CPSR
                push {{r2, r3}}
                push {{r12, lr}}
                subs sp, #8
                push {{r0, r1}}

                cpsid i
                b {push_second_level_state_and_dispatch}

            0:        # YieldReturn
                ",
                push_second_level_state_and_dispatch =
                    sym Self::push_second_level_state_and_dispatch::<Traits>,
                out("r2") _,
                out("r3") _,
            );
        }
    }

    /// Do the following steps:
    ///
    ///  - **Don't** push the first-level state.
    ///  - If the current task is not an idle task,
    ///     - Push the second-level state.
    ///     - Store SP to the current task's `TaskState`.
    ///  - **`dispatch:`** (alternate entry point)
    ///     - Call [`r3_kernel::PortToKernel::choose_running_task`].
    ///     - Restore SP from the next scheduled task's `TaskState`.
    ///  - If there's no task to schedule, branch to [`Self::idle_task`].
    ///  - Pop the second-level state of the next scheduled task.
    ///  - **`pop_first_level_state:`** (alternate entry point)
    ///     - Pop the first-level state of the next scheduled task.
    ///
    /// # Safety
    ///
    ///  - The processor should be in System mode (task context).
    ///  - If the current task is an idle task, SP should point to the
    ///    first-level state on the current task's stack. Otherwise, SP must be
    ///    zero.
    ///  - This function may overwrite any contents in the main stack.
    ///
    #[naked]
    unsafe extern "C" fn push_second_level_state_and_dispatch<Traits: PortInstance>() -> ! {
        extern "C" fn choose_and_get_next_task<Traits: PortInstance>(
        ) -> Option<&'static TaskCb<Traits>> {
            // Safety: CPU Lock active
            unsafe { Traits::choose_running_task() };

            unsafe { *Traits::state().running_task_ptr() }
        }

        unsafe {
            asm!("
                movw r0, :lower16:{p_port_state}_
                movt r0, :upper16:{p_port_state}_
                ldr r0, [r0]

                # Skip saving the second-level state if the current context
                # is an idle task. Also, in this case, we don't have a stack,
                # but `choose_and_get_next_task` needs one. Therefore we borrow
                # the main stack.
                #
                #   if sp_usr == 0:
                #       <running_task is None>
                #       sp_usr = *main_stack_ptr;
                #   else:
                #       /* ... */
                #   
                #   choose_and_get_next_task();
                #
                tst sp, sp
                ldreq sp, [r0, #{OFFSET_MAIN_STACK}]
                beq {push_second_level_state_and_dispatch}.dispatch

                # Push the second-level context state.
                push {{r4-r11}}

                # Store SP to `TaskState`.
                #
                #    <r0 = &port_state>
                #    r0 = *port_state.running_task_ptr // == running_task
                #    r0.port_task_state.sp = sp_usr
                #
                ldr r0, [r0, #{OFFSET_RUNNING_TASK_PTR}]
                ldr r0, [r0]
                str sp, [r0]

            .global {push_second_level_state_and_dispatch}.dispatch
            {push_second_level_state_and_dispatch}.dispatch:
                # Choose the next task to run. `choose_and_get_next_task`
                # returns the new value of `running_task`.
                bl {choose_and_get_next_task}

                # Restore SP from `TaskState`
                #
                #    <r0 = running_task>
                #    if r0.is_none():
                #        goto idle_task;
                #    
                #    sp_usr = r0.port_task_state.sp
                #
                tst r0, r0
                beq {idle_task}
                ldr sp, [r0]

                # Pop the second-level context state.
                pop {{r4-r11}}

            .global {push_second_level_state_and_dispatch}.pop_first_level_state
            {push_second_level_state_and_dispatch}.pop_first_level_state:
                # Reset the local monitor's state (this will cause a
                # subsequent Store-Exclusive to fail)
                clrex

                # Resume the next task by restoring the first-level state
                #
                #   <[r4-r11, sp_usr] = resumed context>
                #
                #   [r0-r3] = SP_usr[0..4];
                #   r12 = SP_usr[4];
                #   lr = SP_usr[5];
                #   pc = SP_usr[6];
                #   CPSR = SP_usr[7];
                #   SP_usr += 8;
                #
                #   <end of procedure>
                #
                pop {{r0-r3, r12, lr}}
                rfeia sp!
            ",
                choose_and_get_next_task = sym choose_and_get_next_task::<Traits>,
                push_second_level_state_and_dispatch =
                    sym Self::push_second_level_state_and_dispatch::<Traits>,
                idle_task = sym Self::idle_task::<Traits>,
                p_port_state = sym Traits::p_port_state,
                OFFSET_RUNNING_TASK_PTR = const Self::OFFSET_RUNNING_TASK_PTR,
                OFFSET_MAIN_STACK = const Self::OFFSET_MAIN_STACK,
                options(noreturn),
            );
        }
    }

    /// Branch to `push_second_level_state_and_dispatch` if `dispatch_pending`
    /// is set. Otherwise, branch to `pop_first_level_state` (thus skipping the
    /// saving/restoration of second-level states).
    #[naked]
    unsafe extern "C" fn push_second_level_state_and_dispatch_shortcutting<Traits: PortInstance>(
    ) -> ! {
        unsafe {
            asm!("
                # Read `port_state().dispatch_pending`. If it's clear, branch
                # to `NotShortcutting`
                movw r0, :lower16:{p_port_state}_
                movt r0, :upper16:{p_port_state}_
                ldr r0, [r0]
                ldrb r1, [r0, #{OFFSET_DISPATCH_PENDING}]
                tst r1, r1
                bne 0f

                # `dispatch_pending` is clear, meaning we are returning to the
                # same task that the current exception has interrupted.
                #
                # If we are returning to the idle task, branch to `idle_task`
                # directly because `pop_first_level_state` can't handle this
                # case.
                tst sp, sp
                beq {idle_task}

                b {push_second_level_state_and_dispatch}.pop_first_level_state

                # `dispatch_pending` is set, meaning `yield_cpu` was called in
                # an interrupt handler, meaning we might need to return to a
                # different task. Clear `dispatch_pending` and branch to
                # `push_second_level_state_and_dispatch`.
            0:                  # NotShortcutting
                movs r1, #0
                strb r1, [r0, #{OFFSET_DISPATCH_PENDING}]
                b {push_second_level_state_and_dispatch}
            ",
                push_second_level_state_and_dispatch =
                    sym Self::push_second_level_state_and_dispatch::<Traits>,
                idle_task = sym Self::idle_task::<Traits>,
                p_port_state = sym Traits::p_port_state,
                OFFSET_DISPATCH_PENDING = const Self::OFFSET_DISPATCH_PENDING,
                options(noreturn),
            );
        }
    }

    /// Enters an idle loop with IRQs unmasked.
    ///
    /// When context switching to the idle task, you don't need to execute
    /// `clrex`.
    ///
    /// # Safety
    ///
    ///  - The processor should be in System mode (task context).
    ///  - `*Traits::state().running_task_ptr()` should be `None`.
    ///
    #[naked]
    unsafe extern "C" fn idle_task<Traits: PortInstance>() -> ! {
        unsafe {
            asm!(
                "
                movs sp, #0
                cpsie i
            0:
                # Ensure all outstanding memory transactions are complete before
                # halting the processor
                dsb
                wfi
                b 0b
            ",
                options(noreturn),
            );
        }
    }

    #[inline(always)]
    pub unsafe fn exit_and_dispatch<Traits: PortInstance>(
        &'static self,
        _task: &'static TaskCb<Traits>,
    ) -> ! {
        unsafe {
            asm!(
                "
                cpsid i
                b {push_second_level_state_and_dispatch}.dispatch
                ",
                push_second_level_state_and_dispatch =
                    sym Self::push_second_level_state_and_dispatch::<Traits>,
                options(noreturn),
            );
        }
    }

    #[inline(always)]
    pub unsafe fn enter_cpu_lock<Traits: PortInstance>(&self) {
        // TODO: support unmanaged interrupts
        unsafe { asm!("cpsid i") };
    }

    #[inline(always)]
    pub unsafe fn leave_cpu_lock<Traits: PortInstance>(&'static self) {
        unsafe { asm!("cpsie i") };
    }

    pub unsafe fn initialize_task_state<Traits: PortInstance>(
        &self,
        task: &'static TaskCb<Traits>,
    ) {
        let stack = task.attr.stack.as_ptr();
        let mut sp = (stack as *mut u8).wrapping_add(stack.len()) as *mut MaybeUninit<u32>;
        // TODO: Enforce minimum stack size

        let preload_all = cfg!(feature = "preload-registers");

        // First-level state (always saved and restored as part of our exception
        // entry/return sequence)
        let first_level = unsafe {
            sp = sp.wrapping_sub(8);
            slice::from_raw_parts_mut(sp, 8)
        };

        // R0: Parameter to the entry point
        first_level[0] = unsafe { core::mem::transmute(task.attr.entry_param) };
        // R1-R3, R12: Uninitialized
        if preload_all {
            first_level[1] = MaybeUninit::new(0x01010101);
            first_level[2] = MaybeUninit::new(0x02020202);
            first_level[3] = MaybeUninit::new(0x03030303);
            first_level[4] = MaybeUninit::new(0x12121212);
        }
        // LR: The return address
        first_level[5] =
            MaybeUninit::new(<System<Traits> as traits::KernelBase>::raw_exit_task as usize as u32);
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
        if preload_all {
            extra_ctx[0] = MaybeUninit::new(0x04040404);
            extra_ctx[1] = MaybeUninit::new(0x05050505);
            extra_ctx[2] = MaybeUninit::new(0x06060606);
            extra_ctx[3] = MaybeUninit::new(0x07070707);
            extra_ctx[4] = MaybeUninit::new(0x08080808);
            extra_ctx[5] = MaybeUninit::new(0x09090909);
            extra_ctx[6] = MaybeUninit::new(0x10101010);
            extra_ctx[7] = MaybeUninit::new(0x11111111);
        }

        let task_state = &task.port_task_state;
        unsafe { *task_state.sp.get() = sp as _ };
    }

    #[inline(always)]
    pub fn is_cpu_lock_active<Traits: PortInstance>(&self) -> bool {
        let cpsr: u32;
        unsafe { asm!("mrs {}, cpsr", out(reg) cpsr) };
        (cpsr & (1 << 7)) != 0
    }

    #[inline(always)]
    pub fn is_task_context<Traits: PortInstance>(&self) -> bool {
        let cpsr: u32;
        unsafe { asm!("mrs {}, cpsr", out(reg) cpsr) };
        (cpsr & 0xf) == 0xf // System mode
    }

    #[inline]
    pub fn is_interrupt_context<Traits: PortInstance>(&self) -> bool {
        self.is_scheduler_active::<Traits>() && !self.is_task_context::<Traits>()
    }

    #[inline]
    pub fn is_scheduler_active<Traits: PortInstance>(&self) -> bool {
        // `main_stack` is assigned by `dispatch_first_task`
        // [ref:arm_main_stack_assigned_in_dft]
        unsafe { *self.main_stack.get() != 0 }
    }

    /// Implements [`crate::EntryPoint::irq_entry`]
    #[naked]
    pub unsafe extern "C" fn irq_entry<Traits: PortInstance>() -> ! {
        unsafe {
            asm!("
                # Adjust `lr_irq` to get the preferred return address. (The
                # required adjustment is different for each exception type.)
                subs lr, #4

                # Switch back to the background mode. The background mode is
                # indicated by SPSR.M on handler entry.
                #
                #   <[r0-r12, sp_xxx, lr_xxx, SPSR] = background context,
                #    lr_irq = preferred return address>
                #
                #   sp_irq = SPSR
                #   match sp_irq.M:
                #       Supervisor => cps Supervisor (== 0x13)
                #       System => cps System (== 0x1f)
                #
                #   <[r0-r12, sp_xxx, lr_xxx, SPSR] = background context,
                #    lr_irq = preferred return address>
                #
                mrs sp, SPSR
                tst sp, #0x8
                bne 0f
                cps #0x13
                b 1f
            0:          # BackgroundIsTask
                cps #0x1f
            1:          # SwitchToBackgroundEnd

                # Skip saving the first-level state if the background context
                # is an idle task.
                #
                #   if sp_xxx == 0:
                #       <&sp_xxx == &sp_usr, running_task is None>
                #       spsr_saved = 0x8
                #       goto PushFirstLevelStateEnd;
                #   
                #   <&sp_xxx != &sp_usr || running_task is Some(_)>
                #
                tst sp, sp
                it eq
                moveq r1, #8
                beq 0f

                # Save the first-level state to the background context's stack
                # (sp_xxx = SP_usr or sp_svc).
                #
                #   <[r0-r12, sp_xxx, lr_xxx, SPSR] = background context,
                #    lr_irq = preferred return address>
                #
                #   sp_xxx -= 8;
                #   sp_xxx[0..4] = [r0-r3];
                #   sp_xxx[4] = r12;
                #   sp_xxx[5] = lr_xxx;
                #
                #   <r2 = sp_xxx, [r4-r11, sp_xxx, SPSR] = background context,
                #    lr_irq = preferred return address>
                #
                subs sp, #8
                push {{r0-r3, r12, lr}}
                mov r2, sp

                # Switch to IRQ mode. Save the return address to the background
                # context's stack.
                #
                #   sp_xxx[6] = lr_irq;
                #   sp_xxx[7] = SPSR;
                #   spsr_saved = SPSR;
                #
                #   <r1 = spsr_saved, [r4-r11, sp_xxx] = background context>
                #
                cps #0x12
                mov r0, lr
                mrs r1, SPSR
                strd r0, r1, [r2, #24]
            0:     # PushFirstLevelStateEnd

                # Switch to Supervisor mode.
                cps #0x13

                # Align `sp_svc` to 8 bytes and save the original `sp_svc`
                # (this is required by AAPCS). At the same time, save `spsr_saved`
                #
                #   <r1 = spsr_saved>
                #   match sp % 8:
                #       0 =>
                #           sp[-2] = spsr_saved;
                #           sp[-1] = sp;
                #           sp -= 2;
                #       4 =>
                #           sp[-3] = spsr_saved;
                #           sp[-2] = sp;
                #           sp -= 3;
                #
                mov r2, sp
                bic sp, #4
                push {{r1, r2}}

                # Call `handle_irq`
                bl {handle_irq}

                # Restore the original `sp_svc` snd `spsr_saved`
                ldrd ip, sp, [sp]

                # Are we returning to a task context?
                #
                #   <ip = spsr_saved>
                #   match spsr_saved.M:
                #       Supervisor => pass
                #       System =>
                #           goto ReturnToTask;
                #
                tst ip, #0x8
                bne 0f

                # We are returning to an outer interrupt handler. Switching the
                # processor mode or finding the next task to dispatch is
                # unnecessary in this case.
                #
                #   <&sp_xxx == &sp_svc, [r4-r11, sp_xxx] = background context>
                #
                #   [r0-r3] = sp_svc[0..4];
                #   r12 = sp_svc[4];
                #   lr_svc = sp_svc[5];
                #   pc = sp_svc[6];
                #   CPSR = sp_svc[7];
                #   sp_svc += 8;
                #
                #   <end of procedure>
                #
                cpsid i
                clrex
                pop {{r0-r3, r12, lr}}
                rfeia sp!

            0:       # ReturnToTask
                cpsid i

                # Back to System mode...
                cps #0x1f

                # Return to the task context by restoring the first-level and
                # second-level state of the next task.
                b {push_second_level_state_and_dispatch_shortcutting}
                ",
                handle_irq = sym Self::handle_irq::<Traits>,
                push_second_level_state_and_dispatch_shortcutting =
                    sym Self::push_second_level_state_and_dispatch_shortcutting::<Traits>,
                options(noreturn),
            );
        }
    }

    unsafe fn handle_irq<Traits: PortInstance>() {
        // Safety: We are the port, so it's okay to call this
        if let Some(line) = unsafe { Traits::acknowledge_interrupt() } {
            // Now that we have signaled the acknowledgement of the current
            // exception, we can start accepting nested exceptions.
            unsafe { asm!("cpsie i") };

            if let Some(handler) = Traits::INTERRUPT_HANDLERS.get(line) {
                // Safety: The first-level interrupt handler is the only code
                //         allowed to call this
                unsafe { handler() };
            }

            // Safety: We are the port, so it's okay to call this
            unsafe { Traits::end_interrupt(line) };
        }
    }
}

/// Used by `use_port!`
pub const fn validate<Traits: PortInstance>() {}
