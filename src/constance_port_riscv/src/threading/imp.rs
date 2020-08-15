use constance::{
    kernel::{Port, PortToKernel, TaskCb},
    prelude::*,
    utils::{intrusive_list::StaticListHead, Init},
};
use core::{borrow::BorrowMut, cell::UnsafeCell, mem::MaybeUninit, slice};

use crate::{InterruptController, ThreadingOptions};

/// `mstatus` (Machine Status Register)
mod mstatus {
    pub const MIE: usize = 1 << 3;
    pub const MPIE: usize = 1 << 7;
    pub const MPP_M: usize = 0b11 << 11;
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
}

pub struct State {
    dispatch_pending: UnsafeCell<bool>,
    main_stack: UnsafeCell<usize>,
}

unsafe impl Sync for State {}

#[derive(Debug)]
#[repr(C)]
pub struct TaskState {
    // TODO
    sp: UnsafeCell<u32>,
}

unsafe impl Sync for TaskState {}

impl State {
    pub const fn new() -> Self {
        Self {
            dispatch_pending: UnsafeCell::new(false),
            main_stack: UnsafeCell::new(0),
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

        // Safety: We are the port, so it's okay to call this
        unsafe { <System as InterruptController>::init() };

        // Safety: We are the port, so it's okay to call this
        unsafe { <System as PortToKernel>::boot() };
    }

    pub unsafe fn dispatch_first_task<System: PortInstance>(&'static self) -> !
    where
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        debug_assert!(self.is_cpu_lock_active::<System>());

        unsafe {
            llvm_asm!("
                # Save the stack pointer for later use
                sw sp, ($0)

                # `mstatus.MPIE` will be `1` all the time
                li a0, $2
                csrs mstatus, a0

                j Dispatch
                "
            :
            :   "r"(self.main_stack.get())
                // Ensure `Dispatch` is emitted
            ,   "X"(Self::push_second_level_state_and_dispatch::<System> as unsafe fn() -> !)
            ,   "i"(mstatus::MPIE)
            :
            :   "volatile");
            core::hint::unreachable_unchecked();
        }
    }

    pub unsafe fn yield_cpu<System: PortInstance>(&'static self)
    where
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        if !self.is_task_context::<System>() {
            unsafe { self.dispatch_pending.get().write_volatile(true) };
        } else {
            unsafe { Self::yield_cpu_in_task::<System>() }
        }
    }

    #[inline(never)] // avoid symbol collision with `YieldReturn`
    #[naked]
    unsafe fn yield_cpu_in_task<System: PortInstance>()
    where
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        unsafe {
            llvm_asm!("
                # Push the first level context state. The return address is
                # set to `YieldReturn`.
                #
                #   sp -= 17;
                #   sp[1..10] = {t0-t2, a0-a5}
                #   sp[10..16] = {a6-a7, t3-t6}
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
                csrci mstatus, $1

                j $0
                "
            :
            :   "X"(Self::push_second_level_state_and_dispatch::<System> as unsafe fn() -> !)
            ,   "i"(mstatus::MIE)
            :
            :   "volatile"
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
    ///     - Pop the first-level state of the next scheduled task.
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
        let main_stack_ptr = System::port_state().main_stack.get();

        unsafe {
            llvm_asm!("
                # Skip saving the second-level state if the current context
                # is an idle task. Also, in this case, we don't have a stack,
                # but `choose_and_get_next_task` needs one. Therefore we borrow
                # the main stack.
                #
                #   if sp == 0 {
                #       [running_task is None]
                #       sp = *main_stack_ptr;
                #   } else {
                #       /* ... */
                #   }
                #   choose_and_get_next_task();
                #
                beqz sp, DispatchSansStack

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
                #    [a0 = &running_task]
                #    a0 = running_task
                #    r0.port_task_state.sp = sp
                #
                lw a0, (a0)
                sw sp, (a0)

                j Dispatch

            DispatchSansStack:
                lw sp, (a1)

            .global Dispatch
            Dispatch:
                # Choose the next task to run. `choose_and_get_next_task`
                # returns the new value of `running_task`.
                jal $1

                # Restore SP from `TaskState`
                #
                #    [a0 = running_task]
                #    if a0.is_none() {
                #        goto idle_task;
                #    }
                #    sp = a0.port_task_state.sp
                #
                beqz a0, $2
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
                # TODO: clear reservation by issuing a dummy SC

                # mstatus.MPP := M
                li a0, $4
                csrs mstatus, a0

                # Resume the next task by restoring the first-level state
                #
                #   [{a0-a7, t0-t6, sp} = resumed context]
                #
                #   mepc = sp[16];
                #   {t0-t2, a0-a5} = sp[1..10];
                #   {a6-a7, t3-t6} = sp[10..16];
                #   sp += 17;
                #
                #   pc = mepc;
                #   mode = mstatus.MPP;
                #
                #   [end of procedure]
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
            "
            :
            :   "{a0}"(running_task_ptr)
            ,   "X"(choose_and_get_next_task::<System> as extern fn() -> _)
            ,   "X"(Self::idle_task::<System> as unsafe fn() -> !)
            ,   "{a1}"(main_stack_ptr)
            ,   "i"(mstatus::MPP_M)
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
        //  - The compiled code does not trash any registers in the second-level
        //    state before entering the inline assembly code below.
        let dispatch_pending_ptr = System::port_state().dispatch_pending.get();

        unsafe {
            llvm_asm!("
                # Read `dispatch_pending`
                lb a1, (a0)
                bnez a1, NotShortcutting

                # `dispatch_pending` is clear, meaning we are returning to the
                # same task that the current exception has interrupted.
                #
                # If we are returning to the idle task, branch to `idle_task`
                # directly because `PopFirstLevelState` can't handle this case.
                beqz sp, $2

                j PopFirstLevelState

                # `dispatch_pending` is set, meaning `yield_cpu` was called in
                # an interrupt handler, meaning we might need to return to a
                # different task. Clear `dispatch_pending` and branch to
                # `push_second_level_state_and_dispatch`.
            NotShortcutting:
                sb zero, (a0)
                j $1
            "
            :
            :   "{a0}"(dispatch_pending_ptr)
            ,   "X"(Self::push_second_level_state_and_dispatch::<System> as unsafe fn() -> !)
            ,   "X"(Self::idle_task::<System> as unsafe fn() -> !)
            :
            :   "volatile");
            core::hint::unreachable_unchecked();
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
    ///  - `*System::state().running_task_ptr()` should be `None`.
    ///
    #[naked]
    unsafe fn idle_task<System: PortInstance>() -> ! {
        unsafe {
            llvm_asm!("
                mv sp, zero

                # MIE := 1
                csrsi mstatus, $0
            IdleLoop:
                wfi
                j IdleLoop
            "
            :
            :   "i"(mstatus::MIE)
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
                # MIE := 0
                csrci mstatus, $0

                j Dispatch
                "
            :
            :   "i"(mstatus::MIE)
            :
            :   "volatile");
            core::hint::unreachable_unchecked();
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
        // TODO: Implement a more reliable method
        let sp: usize;
        unsafe { llvm_asm!("mv $0, sp":"=r"(sp)) };

        let main_stack = unsafe { *self.main_stack.get() };

        sp > main_stack || sp <= main_stack - 512
    }
}

/// Used by `use_port!`
pub const fn validate<System: PortInstance>() {}
