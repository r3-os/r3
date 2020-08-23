use constance::{
    kernel::{
        cfg::InterruptHandlerFn, ClearInterruptLineError, EnableInterruptLineError, InterruptNum,
        InterruptPriority, PendInterruptLineError, Port, PortToKernel, QueryInterruptLineError,
        SetInterruptLinePriorityError, TaskCb,
    },
    prelude::*,
    utils::{intrusive_list::StaticListHead, Init},
};
use core::{
    borrow::BorrowMut, cell::UnsafeCell, hint::unreachable_unchecked, mem::MaybeUninit, slice,
};

use crate::{InterruptController, ThreadingOptions, INTERRUPT_PLATFORM_START, INTERRUPT_SOFTWARE};

/// `mstatus` (Machine Status Register)
mod mstatus {
    pub const MIE: usize = 1 << 3;
    pub const MPIE: usize = 1 << 7;
    pub const MPP_M: usize = 0b11 << 11;

    #[inline(always)]
    pub fn clear_i<const VALUE: usize>() {
        unsafe { asm!("csrci mstatus, {}", const VALUE) };
    }

    #[inline(always)]
    pub fn set_i<const VALUE: usize>() {
        unsafe { asm!("csrsi mstatus, {}", const VALUE) };
    }
}
/// `mcause` (Machine Cause Register)
#[allow(non_upper_case_globals)]
#[allow(dead_code)]
mod mcause {
    pub const Interrupt: usize = usize::MAX - usize::MAX / 2;
    pub const ExceptionCode_MASK: usize = usize::MAX / 2;

    #[inline(always)]
    pub fn read() -> usize {
        let read: usize;
        unsafe { asm!("csrr {}, mcause", lateout(reg) read) };
        read
    }
}

/// `mip` (Machine Interrupt Pending)
mod mip {
    /// Machine Software Interrupt Pending
    pub const MSIP: usize = 1 << 3;
    /// Machine Timer Interrupt Pending
    pub const MTIP: usize = 1 << 7;
    /// Machine External Interrupt Pending
    pub const MEIP: usize = 1 << 11;

    #[inline(always)]
    pub fn read() -> usize {
        let read: usize;
        unsafe { asm!("csrr {}, mip", lateout(reg) read) };
        read
    }

    #[inline(always)]
    pub fn clear(value: usize) {
        unsafe { asm!("csrc mip, {}", in(reg) value) };
    }

    #[inline(always)]
    pub fn set(value: usize) {
        unsafe { asm!("csrs mip, {}", in(reg) value) };
    }
}

/// `mip` (Machine Interrupt Enable)
mod mie {
    /// Machine Software Interrupt Enable
    pub const MSIE: usize = 1 << 3;
    /// Machine Timer Interrupt Enable
    pub const MTIE: usize = 1 << 7;
    /// Machine External Interrupt Enable
    pub const MEIE: usize = 1 << 11;

    #[inline(always)]
    pub fn fetch_clear(value: usize) -> usize {
        let read: usize;
        unsafe { asm!("csrrc {}, mie, {}", lateout(reg) read, in(reg) value) };
        read
    }

    #[inline(always)]
    pub fn clear(value: usize) {
        unsafe { asm!("csrc mie, {}", in(reg) value) };
    }

    #[inline(always)]
    pub fn set(value: usize) {
        unsafe { asm!("csrs mie, {}", in(reg) value) };
    }
}

/// Implemented on a system type by [`use_port!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_port!`].
pub unsafe trait PortInstance:
    Kernel + Port<PortTaskState = TaskState> + ThreadingOptions + InterruptController
{
    fn port_state() -> &'static State;

    const INTERRUPT_SOFTWARE_HANDLER: Option<InterruptHandlerFn>;
    const INTERRUPT_TIMER_HANDLER: Option<InterruptHandlerFn>;
    const INTERRUPT_EXTERNAL_HANDLER: Option<InterruptHandlerFn>;

    const USE_INTERRUPT_SOFTWARE: bool = Self::INTERRUPT_SOFTWARE_HANDLER.is_some();
    const USE_INTERRUPT_TIMER: bool = Self::INTERRUPT_TIMER_HANDLER.is_some();
    const USE_INTERRUPT_EXTERNAL: bool = Self::INTERRUPT_EXTERNAL_HANDLER.is_some();
}

static mut DISPATCH_PENDING: bool = false;

static mut MAIN_STACK: usize = 0;

/// The current nesting level minus one.
///
/// The valid range is `-1..=isize::MAX`. The current context is a task
/// context iff `INTERRUPT_NESTING == -1`.
///
/// `is_task_context` is supposed to return `false` in the main
/// thread (which is a boot context and not a task context). For
/// this reason, `INTERRUPT_NESTING` is initialized as `0`. This
/// doesn't reflect the actual nesting level, but it doesn't matter
/// because interrupts are disabled during booting.
static mut INTERRUPT_NESTING: isize = 0;

pub struct State {}

unsafe impl Sync for State {}

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

        // Safety: We are the port, so it's okay to call this
        unsafe { <System as InterruptController>::init() };

        // Enable local interrupts
        {
            let mut clear_set = [0usize; 2];
            clear_set[System::USE_INTERRUPT_SOFTWARE as usize] |= mie::MSIE;
            clear_set[System::USE_INTERRUPT_TIMER as usize] |= mie::MTIE;
            clear_set[System::USE_INTERRUPT_EXTERNAL as usize] |= mie::MEIE;
            if clear_set[0] != 0 {
                mie::clear(clear_set[0]);
            }
            if clear_set[1] != 0 {
                mie::set(clear_set[1]);
            }
        }

        // Safety: We are the port, so it's okay to call this
        unsafe { <System as PortToKernel>::boot() };
    }

    pub unsafe fn dispatch_first_task<System: PortInstance>(&'static self) -> !
    where
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        debug_assert!(self.is_cpu_lock_active::<System>());

        // We are going to dispatch the first task and enable interrupts, so
        // set `INTERRUPT_NESTING` to `-1`, indicating that there are no active
        // interrupts and we are in a task context.
        unsafe { INTERRUPT_NESTING = -1 };

        unsafe {
            asm!("
                # Save the stack pointer for later use
                sw sp, ({MAIN_STACK}), a0

                # `mstatus.MPIE` will be `1` all the time
                li a0, {MPIE}
                csrs mstatus, a0

                # `Dispatch` is a part of {push_second_level_state_and_dispatch}
                j Dispatch
                ",
                MAIN_STACK = sym MAIN_STACK,
                // Ensure `Dispatch` is emitted
                push_second_level_state_and_dispatch =
                    sym Self::push_second_level_state_and_dispatch::<System>,
                MPIE = const mstatus::MPIE,
                options(noreturn),
            );
        }
    }

    pub unsafe fn yield_cpu<System: PortInstance>(&'static self)
    where
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        if !self.is_task_context::<System>() {
            unsafe { DISPATCH_PENDING = true };
        } else {
            unsafe { Self::yield_cpu_in_task::<System>() }
        }
    }

    #[inline(never)]
    #[naked]
    unsafe fn yield_cpu_in_task<System: PortInstance>()
    where
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        unsafe {
            asm!("
                # Push the first level context state. The saved `pc` directly
                # points to the current return address. This means the saved
                # `ra` (`sp[0]`) is irrelevant.
                #
                #   sp -= 17;
                #   sp[1..10] = [t0-t2, a0-a5]
                #   sp[10..16] = [a6-a7, t3-t6]
                #   sp[16] = ra
                #
                addi sp, sp, (4 * -17)
                sw t0, (4 * 1)(sp)
                sw t1, (4 * 2)(sp)
                sw t2, (4 * 3)(sp)
                sw a0, (4 * 4)(sp)
                sw a1, (4 * 5)(sp)
                sw a2, (4 * 6)(sp)
                sw a3, (4 * 7)(sp)
                sw a4, (4 * 8)(sp)
                sw a5, (4 * 9)(sp)
                sw a6, (4 * 10)(sp)
                sw a7, (4 * 11)(sp)
                sw t3, (4 * 12)(sp)
                sw t4, (4 * 13)(sp)
                sw t5, (4 * 14)(sp)
                sw t6, (4 * 15)(sp)
                sw ra, (4 * 16)(sp)

                # MIE := 0
                csrci mstatus, {MIE}

                tail {push_second_level_state_and_dispatch}
                ",
                push_second_level_state_and_dispatch =
                    sym Self::push_second_level_state_and_dispatch::<System>,
                MIE = const mstatus::MIE,
            );
        }
    }

    /// Do the following steps:
    ///
    ///  - **Don't** push the first-level state.
    ///  - If the current task is not an idle task,
    ///     - Push the second-level state.
    ///     - Store SP to the current task's `TaskState`.
    ///  - `Dispatch:`
    ///     - Call [`constance::kernel::PortToKernel::choose_running_task`].
    ///     - Restore SP from the next scheduled task's `TaskState`.
    ///  - If there's no task to schedule, branch to [`Self::idle_task`].
    ///  - Pop the second-level state of the next scheduled task.
    ///  - `PopFirstLevelState:`
    ///     - Pop the first-level state of the next thread (task or interrupt
    ///       handler) to run.
    ///
    /// # Safety
    ///
    ///  - If the current task is an idle task, SP should point to the
    ///    first-level state on the current task's stack. Otherwise, SP must be
    ///    zero.
    ///  - This function may overwrite any contents in the main stack.
    ///  - `mstatus.MIE` must be equal to `1`.
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
        //  - The compiled code does not trash any registers in the second-level
        //    state before entering the inline assembly code below.
        let running_task_ptr = System::state().running_task_ptr();

        unsafe {
            asm!("
                # Skip saving the second-level state if the current context
                # is an idle task. Also, in this case, we don't have a stack,
                # but `choose_and_get_next_task` needs one. Therefore we borrow
                # the main stack.
                #
                #   if sp == 0:
                #       <running_task is None>
                #       sp = *main_stack_ptr;
                #   else:
                #       /* ... */
                #
                #   choose_and_get_next_task();
                #
                beqz sp, 0f

                # Push the second-level context state.
                addi sp, sp, (4 * -12)
                sw s0, (4 * 0)(sp)
                sw s1, (4 * 1)(sp)
                sw s2, (4 * 2)(sp)
                sw s3, (4 * 3)(sp)
                sw s4, (4 * 4)(sp)
                sw s5, (4 * 5)(sp)
                sw s6, (4 * 6)(sp)
                sw s7, (4 * 7)(sp)
                sw s8, (4 * 8)(sp)
                sw s9, (4 * 9)(sp)
                sw s10, (4 * 10)(sp)
                sw s11, (4 * 11)(sp)

                # Store SP to `TaskState`.
                #
                #    <a0 = &running_task>
                #    a0 = running_task
                #    r0.port_task_state.sp = sp
                #
                lw a0, (a0)
                sw sp, (a0)

                j Dispatch

            0:
                lw sp, ({MAIN_STACK})

            .global Dispatch
            Dispatch:
                # Choose the next task to run. `choose_and_get_next_task`
                # returns the new value of `running_task`.
                call {choose_and_get_next_task}

                # Restore SP from `TaskState`
                #
                #    <a0 = running_task>
                #
                #    if a0.is_none():
                #        goto idle_task;
                #
                #    sp = a0.port_task_state.sp
                #
                beqz a0, 2f
                lw sp, (a0)

                # Pop the second-level context state.
                lw s0, (4 * 0)(sp)
                lw s1, (4 * 1)(sp)
                lw s2, (4 * 2)(sp)
                lw s3, (4 * 3)(sp)
                lw s4, (4 * 4)(sp)
                lw s5, (4 * 5)(sp)
                lw s6, (4 * 6)(sp)
                lw s7, (4 * 7)(sp)
                lw s8, (4 * 8)(sp)
                lw s9, (4 * 9)(sp)
                lw s10, (4 * 10)(sp)
                lw s11, (4 * 11)(sp)
                addi sp, sp, (4 * 12)

            .global PopFirstLevelState
            PopFirstLevelState:
                # Invalidate any reservation held by this hart (this will cause
                # a subsequent Store-Conditional to fail).
                #
                # > Trap handlers should explicitly clear the reservation if
                # > required (e.g., by using a dummy SC) before executing the
                # > xRET.
                addi a1, sp, -4
                sc.w x0, x0, (a1)

                # mstatus.MPP := M
                li a0, {MPP_M}
                csrs mstatus, a0

                # Resume the next task by restoring the first-level state
                #
                #   <[s0-s11, sp] = resumed context>
                #
                #   mepc = sp[16];
                #   [ra, t0-t2, a0-a5] = sp[0..10];
                #   [a6-a7, t3-t6] = sp[10..16];
                #   sp += 17;
                #
                #   pc = mepc;
                #   mode = mstatus.MPP;
                #
                #   <end of procedure>
                #
                lw a7, (4 * 16)(sp)
                csrw mepc, a7
                lw ra, (4 * 0)(sp)
                lw t0, (4 * 1)(sp)
                lw t1, (4 * 2)(sp)
                lw t2, (4 * 3)(sp)
                lw a0, (4 * 4)(sp)
                lw a1, (4 * 5)(sp)
                lw a2, (4 * 6)(sp)
                lw a3, (4 * 7)(sp)
                lw a4, (4 * 8)(sp)
                lw a5, (4 * 9)(sp)
                lw a6, (4 * 10)(sp)
                lw a7, (4 * 11)(sp)
                lw t3, (4 * 12)(sp)
                lw t4, (4 * 13)(sp)
                lw t5, (4 * 14)(sp)
                lw t6, (4 * 15)(sp)
                addi sp, sp, (4 * 17)
                mret

            2:
                tail {idle_task}
                ",
                choose_and_get_next_task = sym choose_and_get_next_task::<System>,
                idle_task = sym Self::idle_task::<System>,
                MAIN_STACK = sym MAIN_STACK,
                MPP_M = const mstatus::MPP_M,
                in("a0") running_task_ptr,
                options(noreturn)
            );
        }
    }

    /// Branch to `push_second_level_state_and_dispatch` if `DISPATCH_PENDING`
    /// is set. Otherwise, branch to `PopFirstLevelState` (thus skipping the
    /// saving/restoration of second-level states).
    #[naked]
    unsafe fn push_second_level_state_and_dispatch_shortcutting<System: PortInstance>() -> !
    where
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        unsafe {
            asm!("
                # Take a shortcut only if `DISPATCH_PENDING == 0`
                lb a1, ({DISPATCH_PENDING})
                bnez a1, NotShortcutting

                # `DISPATCH_PENDING` is clear, meaning we are returning to the
                # same task that the current exception has interrupted.
                #
                # If we are returning to the idle task, branch to `idle_task`
                # directly because `PopFirstLevelState` can't handle this case.
                beqz sp, 0f

                tail PopFirstLevelState

                # `DISPATCH_PENDING` is set, meaning `yield_cpu` was called in
                # an interrupt handler, meaning we might need to return to a
                # different task. Clear `DISPATCH_PENDING` and branch to
                # `push_second_level_state_and_dispatch`.
            NotShortcutting:
                sb zero, ({DISPATCH_PENDING}), a0
                tail {push_second_level_state_and_dispatch}

            0:
                tail {idle_task}
                ",
                DISPATCH_PENDING = sym DISPATCH_PENDING,
                push_second_level_state_and_dispatch =
                    sym Self::push_second_level_state_and_dispatch::<System>,
                idle_task = sym Self::idle_task::<System>,
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
    ///  - `*System::state().running_task_ptr()` should be `None`.
    ///
    #[naked]
    unsafe fn idle_task<System: PortInstance>() -> ! {
        unsafe {
            asm!("
                mv sp, zero

                # MIE := 1
                csrsi mstatus, {MIE}
            IdleLoop:
                wfi
                j IdleLoop
                ",
                MIE = const mstatus::MIE,
                options(nomem, noreturn, nostack),
            );
        }
    }

    pub unsafe fn exit_and_dispatch<System: PortInstance>(
        &'static self,
        _task: &'static TaskCb<System>,
    ) -> ! {
        unsafe {
            asm!("
                # MIE := 0
                csrci mstatus, {MIE}

                j Dispatch
                ",
                MIE = const mstatus::MIE,
                options(noreturn, nostack),
            );
        }
    }

    #[inline(always)]
    pub unsafe fn enter_cpu_lock<System: PortInstance>(&self) {
        unsafe { riscv::register::mstatus::clear_mie() };
    }

    #[inline(always)]
    pub unsafe fn leave_cpu_lock<System: PortInstance>(&'static self) {
        unsafe { riscv::register::mstatus::set_mie() };
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
            sp = sp.wrapping_sub(17);
            slice::from_raw_parts_mut(sp, 17)
        };

        // ra: The return address
        first_level[0] = MaybeUninit::new(System::exit_task as usize as u32);
        // t0-t2: Uninitialized
        first_level[1] = MaybeUninit::new(0x05050505);
        first_level[2] = MaybeUninit::new(0x06060606);
        first_level[3] = MaybeUninit::new(0x07070707);
        // a0: Parameter to the entry point
        first_level[4] = MaybeUninit::new(task.attr.entry_param as u32);
        // a1-a7: Uninitialized
        first_level[5] = MaybeUninit::new(0x11111111);
        first_level[6] = MaybeUninit::new(0x12121212);
        first_level[7] = MaybeUninit::new(0x13131313);
        first_level[8] = MaybeUninit::new(0x14141414);
        first_level[9] = MaybeUninit::new(0x15151515);
        first_level[10] = MaybeUninit::new(0x16161616);
        first_level[11] = MaybeUninit::new(0x17171717);
        // t3-t6: Uninitialized
        first_level[12] = MaybeUninit::new(0x28282828);
        first_level[13] = MaybeUninit::new(0x29292929);
        first_level[14] = MaybeUninit::new(0x30303030);
        first_level[15] = MaybeUninit::new(0x31313131);
        // pc: The entry point
        first_level[16] = MaybeUninit::new(task.attr.entry_point as usize as u32);

        // Second-level state (saved and restored only when we are doing context
        // switching)
        let extra_ctx = unsafe {
            sp = sp.wrapping_sub(12);
            slice::from_raw_parts_mut(sp, 12)
        };

        // s0-s12: Uninitialized
        extra_ctx[0] = MaybeUninit::new(0x08080808);
        extra_ctx[1] = MaybeUninit::new(0x09090909);
        extra_ctx[2] = MaybeUninit::new(0x18181818);
        extra_ctx[3] = MaybeUninit::new(0x19191919);
        extra_ctx[4] = MaybeUninit::new(0x20202020);
        extra_ctx[5] = MaybeUninit::new(0x21212121);
        extra_ctx[6] = MaybeUninit::new(0x22222222);
        extra_ctx[7] = MaybeUninit::new(0x23232323);
        extra_ctx[8] = MaybeUninit::new(0x24242424);
        extra_ctx[9] = MaybeUninit::new(0x25252525);
        extra_ctx[10] = MaybeUninit::new(0x26262626);
        extra_ctx[11] = MaybeUninit::new(0x27272727);

        let task_state = &task.port_task_state;
        unsafe { *task_state.sp.get() = sp as _ };
    }

    #[inline(always)]
    pub fn is_cpu_lock_active<System: PortInstance>(&self) -> bool {
        !riscv::register::mstatus::read().mie()
    }

    pub fn is_task_context<System: PortInstance>(&self) -> bool {
        unsafe { INTERRUPT_NESTING < 0 }
    }

    pub fn set_interrupt_line_priority<System: PortInstance>(
        &'static self,
        num: InterruptNum,
        priority: InterruptPriority,
    ) -> Result<(), SetInterruptLinePriorityError> {
        if num < INTERRUPT_PLATFORM_START {
            Err(SetInterruptLinePriorityError::BadParam)
        } else {
            // Safety: We are delegating the call in the intended way
            unsafe { <System as InterruptController>::set_interrupt_line_priority(num, priority) }
        }
    }

    #[inline]
    pub fn enable_interrupt_line<System: PortInstance>(
        &'static self,
        num: InterruptNum,
    ) -> Result<(), EnableInterruptLineError> {
        if num < INTERRUPT_PLATFORM_START {
            // Enabling or disabling local interrupt lines is not supported
            Err(EnableInterruptLineError::BadParam)
        } else {
            // Safety: We are delegating the call in the intended way
            unsafe { <System as InterruptController>::enable_interrupt_line(num) }
        }
    }

    #[inline]
    pub fn disable_interrupt_line<System: PortInstance>(
        &self,
        num: InterruptNum,
    ) -> Result<(), EnableInterruptLineError> {
        if num < INTERRUPT_PLATFORM_START {
            // Enabling or disabling local interrupt lines is not supported
            Err(EnableInterruptLineError::BadParam)
        } else {
            // Safety: We are delegating the call in the intended way
            unsafe { <System as InterruptController>::disable_interrupt_line(num) }
        }
    }

    #[inline]
    pub fn pend_interrupt_line<System: PortInstance>(
        &'static self,
        num: InterruptNum,
    ) -> Result<(), PendInterruptLineError> {
        if num == INTERRUPT_SOFTWARE {
            mip::set(mip::MSIP);
            Ok(())
        } else if num < INTERRUPT_PLATFORM_START {
            Err(PendInterruptLineError::BadParam)
        } else {
            // Safety: We are delegating the call in the intended way
            unsafe { <System as InterruptController>::pend_interrupt_line(num) }
        }
    }

    #[inline]
    pub fn clear_interrupt_line<System: PortInstance>(
        &self,
        num: InterruptNum,
    ) -> Result<(), ClearInterruptLineError> {
        if num == INTERRUPT_SOFTWARE {
            mip::clear(mip::MSIP);
            Ok(())
        } else if num < INTERRUPT_PLATFORM_START {
            Err(ClearInterruptLineError::BadParam)
        } else {
            // Safety: We are delegating the call in the intended way
            unsafe { <System as InterruptController>::clear_interrupt_line(num) }
        }
    }

    #[inline]
    pub fn is_interrupt_line_pending<System: PortInstance>(
        &self,
        num: InterruptNum,
    ) -> Result<bool, QueryInterruptLineError> {
        if num < INTERRUPT_PLATFORM_START {
            Ok((mip::read() & (mip::MSIP << (num * 4))) != 0)
        } else {
            // Safety: We are delegating the call in the intended way
            unsafe { <System as InterruptController>::is_interrupt_line_pending(num) }
        }
    }

    /// Implements [`crate::EntryPoint::exception_handler`].
    #[naked]
    #[inline(always)]
    pub unsafe fn exception_handler<System: PortInstance>() -> !
    where
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        unsafe {
            asm!("
                # Skip the stacking of the first-level state if the background
                # context is the idle task.
                #
                #   <[a0-a7, t0-t6, s0-s11, sp] = background context state,
                #    background context ∈ [task, idle task, interrupt]>
                #   if sp == 0:
                #       [background context ∈ [idle task]]
                #       INTERRUPT_NESTING += 1;
                #       goto SwitchToMainStack;
                #
                beqz sp, HandlerEntryForIdleTask

                # Push the first-level state to the background context's stack
                #
                #   <[a0-a7, t0-t6, s0-s11, sp] = background context state,
                #    background context ∈ [task, interrupt], sp != 0>
                #
                #   sp -= 17;
                #   sp[0..10] = [ra, t0-t2, a0-a5];
                #   sp[10..16] = [a6-a7, t3-t6];
                #   sp[16] = mepc
                #
                #   let background_sp = sp;
                #   <[s0-s11] = background context state, sp != 0>
                #
                addi sp, sp, (-4 * 17)
                sw ra, (4 * 0)(sp)
                sw t0, (4 * 1)(sp)
                sw t1, (4 * 2)(sp)
                sw t2, (4 * 3)(sp)
                sw a0, (4 * 4)(sp)
                sw a1, (4 * 5)(sp)
                sw a2, (4 * 6)(sp)
                sw a3, (4 * 7)(sp)
                sw a4, (4 * 8)(sp)
                sw a5, (4 * 9)(sp)
                sw a6, (4 * 10)(sp)
                sw a7, (4 * 11)(sp)
                sw t3, (4 * 12)(sp)
                sw t4, (4 * 13)(sp)
                sw t5, (4 * 14)(sp)
                sw t6, (4 * 15)(sp)
                csrr a0, mepc
                sw a0, (4 * 16)(sp)

                # Increment the nesting count.
                #
                #   <INTERRUPT_NESTING ≥ -1>
                #   INTERRUPT_NESTING += 1;
                #   <INTERRUPT_NESTING ≥ 0>
                #
                la a1, {INTERRUPT_NESTING}
                lw a0, (a1)
                addi a0, a0, 1
                sw a0, (a1)

                # If the background context is an interrupt context, we don't
                # have to switch stacks. However, we still need to re-align
                # `sp`.
                #
                # Note: The minimum value of `INTERRUPT_NESTING` is `-1`. Thus
                # at this point, the minimum value we expect to see is `0`.
                #
                #   if INTERRUPT_NESTING > 0:
                #       <background context ∈ [interrupt]>
                #       goto RealignStack;
                #   else:
                #       <background context ∈ [task]>
                #       goto SwitchToMainStack;
                #
                bnez a0, RealignStack

            SwitchToMainStack:
                # If the background context is a task context, we should switch
                # to `MAIN_STACK`. Meanwhile, push the original `sp` to
                # `MAIN_STACK`.
                #
                #   <INTERRUPT_NESTING == 0, background context ∈ [task, idle task]>
                #   *(MAIN_STACK - 4) = sp;
                #   sp = MAIN_STACK - 4;
                #   <sp[0] == background_sp, sp & 15 == 0, sp != 0>
                #
                la a1, {MAIN_STACK}
                lw a0, (a1)
                addi a0, a0, -16
                sw sp, (a0)
                mv sp, a0

                j RealignStackEnd

            RealignStack:
                # Align `sp` to 16 bytes and save the original `sp`.  `sp` is
                # assumed to be already aligned to a word boundary.
                #
                # The 128-bit alignemnt is required by most of the ABIs defined
                # by the following specification:
                # <https://github.com/riscv/riscv-elf-psabi-doc/blob/master/riscv-elf.md>
                #
                # This can be skipped for the ILP32E calling convention
                # (applicable to RV32E), where `sp` is only required to be
                # aligned to a word boundary.
                #
                #   <INTERRUPT_NESTING > 0, background context ∈ [interrupt]>
                #   *((sp - 4) & !15) = sp
                #   sp = (sp - 4) & !15
                #   <sp[0] == background_sp, sp & 15 == 0, sp != 0>
                #
                mv a0, sp
                addi sp, sp, -4
                andi sp, sp, -16
                sw a0, (sp)

            RealignStackEnd:
                # Call `handle_exception`
                call {handle_exception}

                # Decrement the nesting count.
                #
                #   <INTERRUPT_NESTING ≥ 0>
                #   INTERRUPT_NESTING -= 1;
                #   <INTERRUPT_NESTING ≥ -1>
                #
                la a1, {INTERRUPT_NESTING}
                lw a0, (a1)
                addi a0, a0, -1
                sw a0, (a1)

                # Restore `background_sp`
                lw sp, (sp)

                # Are we returning to an interrupt context?
                #
                # If we are returning to an outer interrupt handler, finding the
                # next task to dispatch is unnecessary, so we can jump straight
                # to `PopFirstLevelState`.
                #
                #   <INTERRUPT_NESTING ≥ -1>
                #   if INTERRUPT_NESTING >= 0:
                #       goto PopFirstLevelState;
                #
                bgez a0, PopFirstLevelStateTrampoline

                # Return to the task context by restoring the first-level and
                # second-level state of the next task.
                tail {push_second_level_state_and_dispatch_shortcutting}

            PopFirstLevelStateTrampoline:
                tail PopFirstLevelState

            HandlerEntryForIdleTask:
                # Increment the nesting count.
                #
                #   <INTERRUPT_NESTING == -1, background context ∈ [idle task]>
                #   INTERRUPT_NESTING += 1;
                #   <INTERRUPT_NESTING == 0>
                #
                la a1, {INTERRUPT_NESTING}
                sw x0, (a1)
                j SwitchToMainStack
                ",
                handle_exception = sym Self::handle_exception::<System>,
                push_second_level_state_and_dispatch_shortcutting =
                    sym Self::push_second_level_state_and_dispatch_shortcutting::<System>,
                INTERRUPT_NESTING = sym INTERRUPT_NESTING,
                MAIN_STACK = sym MAIN_STACK,
                options(noreturn)
            );
        }
    }

    unsafe fn handle_exception<System: PortInstance>()
    where
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        let mcause = mcause::read();
        if (mcause & mcause::Interrupt) == 0 {
            panic!("unhandled exception: {}", mcause);
        }

        let all_local_interrupts = [0, mie::MSIE][System::USE_INTERRUPT_SOFTWARE as usize]
            | [0, mie::MTIE][System::USE_INTERRUPT_TIMER as usize]
            | [0, mie::MEIE][System::USE_INTERRUPT_EXTERNAL as usize];

        // `M[EST]IE` is used to simulate execution priority levels.
        //
        //  | MEIE | MSIE | MTIE | Priority |
        //  | ---- | ---- | ---- | -------- |
        //  |    0 |    0 |    0 |        3 |
        //  |    1 |    0 |    0 |        2 |
        //  |    1 |    1 |    0 |        1 |
        //  |    1 |    1 |    1 | 0 (Task) |
        //
        // First, we raise the execution priority to maximum by clearing all of
        // `M[EST]IE`. Then we lower the execution priority one by one as we
        // skim through the pending flags.
        //
        // We must not lower the execution priority to a background execution
        // priority while interrupts are enabled globally for this can lead to
        // an unbounded stack consumption.
        //
        // The simplified pseudocode is shown below:
        //
        //  let bg_exc_pri = get_exc_pri();
        //  set_exc_pri(3);
        //  enable_interrupts_globally();
        //  for exc_pri in (bg_exc_pri + 1 ..= 3).rev() {
        //      set_exc_pri(exc_pri);
        //      while pending[exc_pri] { handlers[exc_pri](); }
        //  }
        //  disable_interrupts_globally();
        //  set_exc_pri(bg_exc_pri);
        //
        // The actual implementaion is closer to the following:
        //
        //  let bg_exc_pri = get_exc_pri();  // This value is implicit
        //  let mut found_bg_exc_pri;        // Represented by `mie_pending`
        //  set_exc_pri(3);
        //  enable_interrupts_globally();
        //  for exc_pri in (1 ..= 3).rev() {
        //      if exc_pri > bg_exc_pri {
        //          set_exc_pri(exc_pri);
        //          while pending[exc_pri] { handlers[exc_pri](); }
        //          found_bg_exc_pri = exc_pri - 1;
        //      }
        //  }
        //  disable_interrupts_globally();
        //  set_exc_pri(found_bg_exc_pri);
        //
        //
        let old_mie = mie::fetch_clear(all_local_interrupts);
        let mut mie_pending = 0;

        // Re-enable interrupts globally.
        mstatus::set_i::<{ mstatus::MIE }>();

        let mut mip = mip::read();

        // Check the pending flags and call the respective handlers in the
        // descending order of priority.
        if System::USE_INTERRUPT_EXTERNAL && (old_mie & mie::MEIE) != 0 {
            // Safety: `USE_INTERRUPT_EXTERNAL == true`
            let handler = System::INTERRUPT_EXTERNAL_HANDLER
                .unwrap_or_else(|| unsafe { unreachable_unchecked() });

            while (mip & mip::MEIP) != 0 {
                // Safety: The first-level interrupt handler is allowed to call
                //         a second-level interrupt handler
                unsafe { handler() };

                mip = mip::read();
            }

            mie_pending = mie::MEIE;
        }

        if System::USE_INTERRUPT_SOFTWARE && (old_mie & mie::MSIE) != 0 {
            // Safety: `USE_INTERRUPT_SOFTWARE == true`
            let handler = System::INTERRUPT_SOFTWARE_HANDLER
                .unwrap_or_else(|| unsafe { unreachable_unchecked() });

            if System::USE_INTERRUPT_EXTERNAL {
                debug_assert_eq!(mie_pending, mie::MEIE);
                mie::set(mie::MEIE);
            } else {
                debug_assert_eq!(mie_pending, 0);
            }

            while (mip & mip::MSIP) != 0 {
                // Safety: The first-level interrupt handler is allowed to call
                //         a second-level interrupt handler
                unsafe { handler() };

                mip = mip::read();
            }

            mie_pending = mie::MSIE;
        }

        if System::USE_INTERRUPT_TIMER && (old_mie & mie::MTIE) != 0 {
            // Safety: `USE_INTERRUPT_TIMER == true`
            let handler = System::INTERRUPT_TIMER_HANDLER
                .unwrap_or_else(|| unsafe { unreachable_unchecked() });

            if System::USE_INTERRUPT_SOFTWARE {
                debug_assert_eq!(mie_pending, mie::MSIE);
                mie::set(mie::MSIE);
            } else if System::USE_INTERRUPT_EXTERNAL {
                debug_assert_eq!(mie_pending, mie::MEIE);
                mie::set(mie::MEIE);
            } else {
                debug_assert_eq!(mie_pending, 0);
            }

            while (mip & mip::MTIP) != 0 {
                // Safety: The first-level interrupt handler is allowed to call
                //         a second-level interrupt handler
                unsafe { handler() };

                mip = mip::read();
            }

            mie_pending = mie::MTIE;
        }

        // Disable interrupts globally before returning.
        mstatus::clear_i::<{ mstatus::MIE }>();

        debug_assert_ne!(mie_pending, 0);
        mie::set(mie_pending);
    }
}

/// Used by `use_port!`
pub const fn validate<System: PortInstance>() {}
