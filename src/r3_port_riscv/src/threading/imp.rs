use core::{arch::asm, cell::UnsafeCell, hint::unreachable_unchecked, mem::MaybeUninit, slice};
use r3_core::{
    kernel::{
        interrupt::{InterruptHandlerFn, InterruptNum},
        traits, ClearInterruptLineError, EnableInterruptLineError, InterruptPriority,
        PendInterruptLineError, QueryInterruptLineError, SetInterruptLinePriorityError,
    },
    utils::Init,
};
use r3_kernel::{KernelTraits, Port, PortToKernel, System, TaskCb};
use r3_portkit::pptext::pp_asm;

use crate::{
    InterruptController, ThreadingOptions, Timer, INTERRUPT_PLATFORM_START, INTERRUPT_SOFTWARE,
};

/// `XLEN / 8`
const X_SIZE: usize = core::mem::size_of::<usize>();

/// `FLEN / 8`
const F_SIZE: usize = if cfg!(target_feature = "q") {
    16
} else if cfg!(target_feature = "d") {
    8
} else {
    4
};

/// The natural alignment of stored register values in FLS and SLS.
const REG_ALIGN: usize = if F_SIZE > X_SIZE { F_SIZE } else { X_SIZE };

/// The size of FLS.F
const FLSF_SIZE: usize = 20 * F_SIZE + REG_ALIGN;

/// The assembler fragments used by `pp_asm!`. Because of a mysterious macro
/// hygienics behavior, they have to referred to by absolute paths.
///
/// They are marked as `pub` to be used by `r3_port_riscv_test_driver`.
#[rustfmt::skip]
#[doc(hidden)]
pub mod asm_inc {
    // define_load_store - defines the macros for XLEN-bit load/store
    // -----------------------------------------------------------------
    #[cfg(target_pointer_width = "128")]
    pub macro define_load_store() {r"
        .ifndef load_store_defined
            .set load_store_defined, 1
            .macro LOAD p:vararg
                lq \p
            .endm
            .macro STORE p:vararg
                sq \p
            .endm
            .macro C.LOAD p:vararg
                c.lq \p
            .endm
            .macro C.STORE p:vararg
                c.sq \p
            .endm
        .endif
    "}

    #[cfg(target_pointer_width = "64")]
    pub macro define_load_store() {r"
        .ifndef load_store_defined
            .set load_store_defined, 1
            .macro LOAD p:vararg
                ld \p
            .endm
            .macro STORE p:vararg
                sd \p
            .endm
            .macro C.LOAD p:vararg
                c.ld \p
            .endm
            .macro C.STORE p:vararg
                c.sd \p
            .endm
        .endif
    "}

    #[cfg(target_pointer_width = "32")]
    pub macro define_load_store() {r"
        .ifndef load_store_defined
            .set load_store_defined, 1
            .macro LOAD p:vararg
                lw \p
            .endm
            .macro STORE p:vararg
                sw \p
            .endm
            .macro C.LOAD p:vararg
                c.lw \p
            .endm
            .macro C.STORE p:vararg
                c.sw \p
            .endm
        .endif
    "}

    // define_fload_fstore - defines the macros for FLEN-bit load/store
    // -----------------------------------------------------------------
    #[cfg(target_feature = "q")]
    pub macro define_fload_fstore() {r"
        .ifndef fload_store_defined
            .set fload_store_defined, 1
            .macro FLOAD rd mem
                flq \rd, \mem
            .endm
            .macro FSTORE rs mem
                fsq \rs, \mem
            .endm
        .endif
    "}

    #[cfg(all(target_feature = "d", not(target_feature = "q")))]
    pub macro define_fload_fstore() {r"
        .ifndef fload_store_defined
            .set fload_store_defined, 1
            .macro FLOAD rd mem
                fld \rd, \mem
            .endm
            .macro FSTORE rs mem
                fsd \rs, \mem
            .endm
        .endif
    "}

    #[cfg(all(not(target_feature = "d"), not(target_feature = "q")))]
    pub macro define_fload_fstore() {r"
        .ifndef fload_store_defined
            .set fload_store_defined, 1
            .macro FLOAD rd mem
                flw \rd, \mem
            .endm
            .macro FSTORE rs mem
                fsw \rs, \mem
            .endm
        .endif
    "}
}

// Should be defined after `asm_inc` for a "cannot determine resolution for the
// macro" error not to occur
mod instemu;

mod csr;
use csr::{CsrAccessor as _, CsrSetAccess as _};
#[doc(hidden)] // used by macro
pub use csr::{CsrSet, NumTy};

/// The part of `xstatus` which is specific to each thread.
///
/// `xstatus_part` is only used if `cfg!(target_feature = "f")`. `xstatus_part`
/// is undefined otherwise.
#[allow(dead_code)]
const XSTATUS_PART_MASK: usize = csr::XSTATUS_FS_1;

/// Implemented on a system type by [`use_port!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_port!`].
pub unsafe trait PortInstance:
    KernelTraits + Port<PortTaskState = TaskState> + ThreadingOptions + InterruptController + Timer
{
    fn port_state() -> &'static State;

    const INTERRUPT_SOFTWARE_HANDLER: Option<InterruptHandlerFn>;
    const INTERRUPT_TIMER_HANDLER: Option<InterruptHandlerFn>;
    const INTERRUPT_EXTERNAL_HANDLER: Option<InterruptHandlerFn>;

    const USE_INTERRUPT_SOFTWARE: bool = Self::INTERRUPT_SOFTWARE_HANDLER.is_some();
    const USE_INTERRUPT_TIMER: bool = Self::INTERRUPT_TIMER_HANDLER.is_some();
    const USE_INTERRUPT_EXTERNAL: bool = Self::INTERRUPT_EXTERNAL_HANDLER.is_some();

    type Csr: csr::CsrSetAccess;

    /// Validated privilege level encoding.
    type Priv: csr::Num;
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
static mut INTERRUPT_NESTING: i32 = 0;

pub struct State {}

unsafe impl Sync for State {}

#[derive(Debug)]
#[repr(C)]
pub struct TaskState {
    sp: UnsafeCell<usize>,
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
    pub unsafe fn port_boot<Traits: PortInstance>(&self) -> ! {
        unsafe { self.enter_cpu_lock::<Traits>() };

        // Enable FPU
        if cfg!(target_feature = "f") {
            // FS = 0b?1 (Initial or Dirty)
            Traits::Csr::xstatus().set(csr::XSTATUS_FS_0);
        }

        // Safety: We are the port, so it's okay to call this
        unsafe { <Traits as InterruptController>::init() };

        // Safety: We are the port, so it's okay to call this
        unsafe { <Traits as Timer>::init() };

        // Enable local interrupts
        {
            let mut clear_set = [0usize; 2];
            clear_set[Traits::USE_INTERRUPT_SOFTWARE as usize] |= Traits::Csr::XIE_XSIE;
            clear_set[Traits::USE_INTERRUPT_TIMER as usize] |= Traits::Csr::XIE_XTIE;
            clear_set[Traits::USE_INTERRUPT_EXTERNAL as usize] |= Traits::Csr::XIE_XEIE;
            if clear_set[0] != 0 {
                Traits::Csr::xie().clear(clear_set[0]);
            }
            if clear_set[1] != 0 {
                Traits::Csr::xie().set(clear_set[1]);
            }
        }

        // Safety: We are the port, so it's okay to call this
        unsafe { <Traits as PortToKernel>::boot() };
    }

    pub unsafe fn dispatch_first_task<Traits: PortInstance>(&'static self) -> ! {
        debug_assert!(self.is_cpu_lock_active::<Traits>());

        // We are going to dispatch the first task and enable interrupts, so
        // set `INTERRUPT_NESTING` to `-1`, indicating that there are no active
        // interrupts and we are in a task context.
        unsafe { INTERRUPT_NESTING = -1 };

        unsafe {
            pp_asm!("
            "   crate::threading::imp::asm_inc::define_load_store!()              "
                # Save the stack pointer for later use
                STORE sp, ({MAIN_STACK}), a0

                # `xstatus.XPIE` will be `1` all the time except in a software
                # exception handler
                li a0, " crate::threading::imp::csr::csrexpr!(XSTATUS_XPIE) "
                csrs " crate::threading::imp::csr::csrexpr!(XSTATUS) ", a0

                tail {push_second_level_state_and_dispatch}.dispatch
                ",
                MAIN_STACK = sym MAIN_STACK,
                push_second_level_state_and_dispatch =
                    sym Self::push_second_level_state_and_dispatch::<Traits>,
                PRIV = sym <<Traits as PortInstance>::Priv as csr::Num>::value,
                options(noreturn),
            );
        }
    }

    #[inline]
    pub unsafe fn yield_cpu<Traits: PortInstance>(&'static self) {
        if !self.is_task_context::<Traits>() {
            unsafe { DISPATCH_PENDING = true };
        } else {
            // `yield_cpu_in_task` does not clobber any registers except
            // for `ra`
            unsafe {
                asm!("
                    call {yield_cpu_in_task}
                    ",
                    yield_cpu_in_task = sym Self::yield_cpu_in_task::<Traits>,
                    out("ra") _,
                );
            }
        }
    }

    #[naked]
    unsafe extern "C" fn yield_cpu_in_task<Traits: PortInstance>() {
        unsafe {
            pp_asm!("
            "   crate::threading::imp::asm_inc::define_load_store!()              "
            "   crate::threading::imp::asm_inc::define_fload_fstore!()              "

                # Push the first level context state. The saved `pc` directly
                # points to the current return address. This means the saved
                # `ra` (`sp[0]`) is irrelevant.
                #
                #   sp -= 17;
                #   sp[1..10] = [t0-t2, a0-a5]
                #   sp[10..16] = [a6-a7, t3-t6]
                #   sp[16] = ra
                #
                addi sp, sp, ({X_SIZE} * -17)
                STORE t0, ({X_SIZE} * 1)(sp)
                STORE t1, ({X_SIZE} * 2)(sp)
                STORE t2, ({X_SIZE} * 3)(sp)
                STORE a0, ({X_SIZE} * 4)(sp)
                STORE a1, ({X_SIZE} * 5)(sp)
                STORE a2, ({X_SIZE} * 6)(sp)
                STORE a3, ({X_SIZE} * 7)(sp)
                STORE a4, ({X_SIZE} * 8)(sp)
                STORE a5, ({X_SIZE} * 9)(sp)
                STORE a6, ({X_SIZE} * 10)(sp)
                STORE a7, ({X_SIZE} * 11)(sp)
                STORE t3, ({X_SIZE} * 12)(sp)
                STORE t4, ({X_SIZE} * 13)(sp)
                STORE t5, ({X_SIZE} * 14)(sp)
                STORE t6, ({X_SIZE} * 15)(sp)
                STORE ra, ({X_SIZE} * 16)(sp)

                # XIE := 0
                csrrci a0, " crate::threading::imp::csr::csrexpr!(XSTATUS) ",       "
                    crate::threading::imp::csr::csrexpr!(XSTATUS_XIE)               "

            "   if cfg!(target_feature = "f") {                                     "
                    # If FP registers are in use, push FLS.F
                    #
                    #   <a2 = xstatus_part>
                    #   if xstatus_part.FS[1] != 0:
                    #       sp: *mut FlsF;
                    #       sp -= 1;
                    #       sp['ft0'-'ft7'] = [ft0-ft7];
                    #       sp['fa0'-'fa7'] = [fa0-fa7];
                    #       sp['ft8'-'ft11'] = [ft8-ft11];
                    #       sp.fcsr = fcsr;
                    #   <a0 = xstatus_part>
                    #
                    li a1, {FS_1}
                    and a1, a1, a0
                    beqz a1, 0f      # → PushFLSFEnd

                    csrr a1, fcsr

                    addi sp, sp, -{FLSF_SIZE}
                    FSTORE ft0, ({F_SIZE} * 0)(sp)
                    FSTORE ft1, ({F_SIZE} * 1)(sp)
                    FSTORE ft2, ({F_SIZE} * 2)(sp)
                    FSTORE ft3, ({F_SIZE} * 3)(sp)
                    FSTORE ft4, ({F_SIZE} * 4)(sp)
                    FSTORE ft5, ({F_SIZE} * 5)(sp)
                    FSTORE ft6, ({F_SIZE} * 6)(sp)
                    FSTORE ft7, ({F_SIZE} * 7)(sp)
                    FSTORE fa0, ({F_SIZE} * 8)(sp)
                    FSTORE fa1, ({F_SIZE} * 9)(sp)
                    FSTORE fa2, ({F_SIZE} * 10)(sp)
                    FSTORE fa3, ({F_SIZE} * 11)(sp)
                    FSTORE fa4, ({F_SIZE} * 12)(sp)
                    FSTORE fa5, ({F_SIZE} * 13)(sp)
                    FSTORE fa6, ({F_SIZE} * 14)(sp)
                    FSTORE fa7, ({F_SIZE} * 15)(sp)
                    FSTORE ft8, ({F_SIZE} * 16)(sp)
                    FSTORE ft9, ({F_SIZE} * 17)(sp)
                    FSTORE ft10, ({F_SIZE} * 18)(sp)
                    FSTORE ft11, ({F_SIZE} * 19)(sp)
                    STORE a1, ({F_SIZE} * 20)(sp)
                0:      # PushFLSFEnd
            "   } else {                                                            "
                    # unused: {F_SIZE} {FS_1} {FLSF_SIZE}
            "   }                                                                   "

                tail {push_second_level_state_and_dispatch}.not_shortcutting
                ",
                push_second_level_state_and_dispatch =
                    sym Self::push_second_level_state_and_dispatch::<Traits>,
                PRIV = sym <<Traits as PortInstance>::Priv as csr::Num>::value,
                FS_1 = const csr::XSTATUS_FS_1,
                X_SIZE = const X_SIZE,
                F_SIZE = const F_SIZE,
                FLSF_SIZE = const FLSF_SIZE,
                options(noreturn),
            );
        }
    }

    /// The central procedure for task dispatching.
    ///
    /// The procedure does the following:
    ///
    ///  - **Don't** push the first-level state.
    ///  - If `DISPATCH_PENDING == 0`,
    ///     - If the current task is not the idle task, go to
    ///       `pop_first_level_state`.
    ///     - Otherwise, branch to the idle task loop.
    ///  - **`not_shortcutting:`** (alternate entry point)
    ///  - If the current task is not the idle task,
    ///     - Push the second-level state.
    ///     - Store SP to the current task's `TaskState`.
    ///  - If the current task is the idle task,
    ///     - Update SP to point to the main stack. In this case, **this
    ///       procedure may overwrite any contents in the main stack.**
    ///  - **`dispatch:`** (alternate entry point)
    ///  - Call [`r3_kernel::PortToKernel::choose_running_task`].
    ///  - Restore SP from the next scheduled task's `TaskState`.
    ///  - If there's no task to schedule, branch to the idle task loop.
    ///  - Pop the second-level state of the next scheduled task.
    ///  - **`pop_first_level_state:`** (alternate entry point)
    ///  - Pop the first-level state of the next thread (task or interrupt
    ///    handler) to run.
    ///
    /// # Safety
    ///
    /// All entry points:
    ///
    ///  - `xstatus.XIE` must be equal to `1`.
    ///
    /// All entry points but `dispatch`:
    ///
    ///  - If the current task is a task, SP should point to the
    ///    first-level state on the current task's stack. Otherwise, SP must be
    ///    zero.
    ///  - In a configuration that uses `xstatus_part`, `a0` must include the
    ///    `xstatus_part` of the current task.
    ///
    /// `dispatch`:
    ///
    ///  - SP must point to a valid stack.
    ///
    /// `pop_first_level_state`:
    ///
    ///  - The current task must not be the idle task.
    ///
    #[naked]
    unsafe extern "C" fn push_second_level_state_and_dispatch<Traits: PortInstance>() -> ! {
        #[repr(C)]
        struct A0A1<S, T>(S, T);

        extern "C" fn choose_and_get_next_task<Traits: PortInstance>(
        ) -> A0A1<MaybeUninit<usize>, Option<&'static TaskCb<Traits>>> {
            // Safety: CPU Lock active
            unsafe { Traits::choose_running_task() };

            A0A1(MaybeUninit::uninit(), unsafe {
                *Traits::state().running_task_ptr()
            })
        }

        extern "C" fn get_running_task<Traits: PortInstance>(
            a0: usize,
        ) -> A0A1<usize, Option<&'static TaskCb<Traits>>> {
            A0A1(
                a0, // preserve `a0`
                unsafe { *Traits::state().running_task_ptr() },
            )
        }

        unsafe {
            pp_asm!("
            "   crate::threading::imp::asm_inc::define_load_store!()              "
            "   crate::threading::imp::asm_inc::define_fload_fstore!()              "

                # <a0 = xstatus_part>
                # Take a shortcut only if `DISPATCH_PENDING == 0`
                lb a1, ({DISPATCH_PENDING})
                bnez a1, 0f

                # `DISPATCH_PENDING` is clear, meaning we are returning to the
                # same task that the current exception has interrupted.
                #
                # If we are returning to the idle task, branch to `idle_task`
                # directly because `pop_first_level_state` can't handle this case.
                beqz sp, {push_second_level_state_and_dispatch}.idle_task

                j {push_second_level_state_and_dispatch}.pop_first_level_state

            0:
                # `DISPATCH_PENDING` is set, meaning `yield_cpu` was called in
                # an interrupt handler, meaning we might need to return to a
                # different task. Clear `DISPATCH_PENDING` and proceeed to
                # `not_shortcutting`.
                sb zero, ({DISPATCH_PENDING}), a2

            .global {push_second_level_state_and_dispatch}.not_shortcutting
            {push_second_level_state_and_dispatch}.not_shortcutting:
                # <a0 = xstatus_part>

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
                beqz sp, 1f

                # Read `running_task` earlier to hide the load-use latency.
                call {get_running_task}

                # Push the SLS.X.
                addi sp, sp, ({X_SIZE} * -12)
                STORE s0, ({X_SIZE} * 0)(sp)
                STORE s1, ({X_SIZE} * 1)(sp)
                STORE s2, ({X_SIZE} * 2)(sp)
                STORE s3, ({X_SIZE} * 3)(sp)
                STORE s4, ({X_SIZE} * 4)(sp)
                STORE s5, ({X_SIZE} * 5)(sp)
                STORE s6, ({X_SIZE} * 6)(sp)
                STORE s7, ({X_SIZE} * 7)(sp)
                STORE s8, ({X_SIZE} * 8)(sp)
                STORE s9, ({X_SIZE} * 9)(sp)
                STORE s10, ({X_SIZE} * 10)(sp)
                STORE s11, ({X_SIZE} * 11)(sp)

                # The following branch checks the following conditions, which
                # are coincidentally identical, at the same time
                #
                #  - Is it possible for FP registers to be in use?
                #  - Do we use `xstatus_part`?
                #
            "   if cfg!(target_feature = "f") {                                     "
                    # If FP registers are in use, push SLS.F.
                    #
                    #   <a0 = xstatus_part>
                    #   if xstatus_part.FS[1] != 0:
                    #       sp: *mut FReg;
                    #       sp -= 12;
                    #       sp[0..12] = [fs0-fs11];
                    #   <a0 = xstatus_part>
                    #
                    li a2, {FS_1}
                    and a2, a2, a0
                    beqz a2, 0f      # → PushSLSFEnd

                    addi sp, sp, (-{F_SIZE} * 12)
                    FSTORE fs0, ({F_SIZE} * 0)(sp)
                    FSTORE fs1, ({F_SIZE} * 1)(sp)
                    FSTORE fs2, ({F_SIZE} * 2)(sp)
                    FSTORE fs3, ({F_SIZE} * 3)(sp)
                    FSTORE fs4, ({F_SIZE} * 4)(sp)
                    FSTORE fs5, ({F_SIZE} * 5)(sp)
                    FSTORE fs6, ({F_SIZE} * 6)(sp)
                    FSTORE fs7, ({F_SIZE} * 7)(sp)
                    FSTORE fs8, ({F_SIZE} * 8)(sp)
                    FSTORE fs9, ({F_SIZE} * 9)(sp)
                    FSTORE fs10, ({F_SIZE} * 10)(sp)
                    FSTORE fs11, ({F_SIZE} * 11)(sp)
                0:      # PushSLSFEnd

                    # Push `xstatus_part`
                    addi sp, sp, -{X_SIZE}
                    STORE a0, (sp)
            "   } else {                                                            "
                    # unused: {F_SIZE} {FS_1}
            "   }                                                                   "

                # Store SP to `TaskState`.
                #
                #    <a1 = running_task>
                #    a1.port_task_state.sp = sp
                #
                STORE sp, (a1)

                j {push_second_level_state_and_dispatch}.dispatch

            1:
                LOAD sp, ({MAIN_STACK})

            .global {push_second_level_state_and_dispatch}.dispatch
            {push_second_level_state_and_dispatch}.dispatch:
                # Choose the next task to run. `choose_and_get_next_task`
                # returns the new value of `running_task`.
                call {choose_and_get_next_task}

                # Restore SP from `TaskState`
                #
                #    <a1 = running_task>
                #
                #    if a1.is_none():
                #        goto idle_task;
                #
                #    sp = a1.port_task_state.sp
                #
                beqz a1, {push_second_level_state_and_dispatch}.idle_task
                LOAD sp, (a1)

                # The following branch checks the following conditions, which
                # are coincidentally identical, at the same time
                #
                #  - Is it possible for FP registers to be in use?
                #  - Do we use `xstatus_part`?
                #
            "   if cfg!(target_feature = "f") {                                     "
                    # Pop `xstatus_part`
                    LOAD a0, (sp)
                    addi sp, sp, {X_SIZE}

                    # If FP registers are in use, pop SLS.F.
                    #
                    #   <a0 = xstatus_part>
                    #   if xstatus_part.FS[1] != 0:
                    #       sp: *mut FReg;
                    #       [fs0-fs11] = sp[0..12];
                    #       sp += 12;
                    #   <a0 = xstatus_part>
                    #
                    li a2, {FS_1}
                    and a2, a2, a0
                    beqz a2, 0f      # → PopSLSFEnd

                    FLOAD fs0, ({F_SIZE} * 0)(sp)
                    FLOAD fs1, ({F_SIZE} * 1)(sp)
                    FLOAD fs2, ({F_SIZE} * 2)(sp)
                    FLOAD fs3, ({F_SIZE} * 3)(sp)
                    FLOAD fs4, ({F_SIZE} * 4)(sp)
                    FLOAD fs5, ({F_SIZE} * 5)(sp)
                    FLOAD fs6, ({F_SIZE} * 6)(sp)
                    FLOAD fs7, ({F_SIZE} * 7)(sp)
                    FLOAD fs8, ({F_SIZE} * 8)(sp)
                    FLOAD fs9, ({F_SIZE} * 9)(sp)
                    FLOAD fs10, ({F_SIZE} * 10)(sp)
                    FLOAD fs11, ({F_SIZE} * 11)(sp)
                    addi sp, sp, {F_SIZE} * 12
                0:      # PopSLSFEnd
            "   }                                                                   "

                # Pop the second-level context state.
                LOAD s0, ({X_SIZE} * 0)(sp)
                LOAD s1, ({X_SIZE} * 1)(sp)
                LOAD s2, ({X_SIZE} * 2)(sp)
                LOAD s3, ({X_SIZE} * 3)(sp)
                LOAD s4, ({X_SIZE} * 4)(sp)
                LOAD s5, ({X_SIZE} * 5)(sp)
                LOAD s6, ({X_SIZE} * 6)(sp)
                LOAD s7, ({X_SIZE} * 7)(sp)
                LOAD s8, ({X_SIZE} * 8)(sp)
                LOAD s9, ({X_SIZE} * 9)(sp)
                LOAD s10, ({X_SIZE} * 10)(sp)
                LOAD s11, ({X_SIZE} * 11)(sp)
                addi sp, sp, ({X_SIZE} * 12)

            .global {push_second_level_state_and_dispatch}.pop_first_level_state
            {push_second_level_state_and_dispatch}.pop_first_level_state:
                # <a0 = xstatus_part>

            "   if cfg!(target_feature = "f") {                                     "
                    # If FP registers were in use, pop FLS.F. Loading FP regs
                    # will implicitly set `xstatus.FS[1]`.
                    #
                    #   <a0 = xstatus_part>
                    #   if xstatus_part.FS[1] != 0:
                    #       sp: *mut FlsF;
                    #       [ft0-ft7] = sp['ft0'-'ft7'];
                    #       [fa0-fa7] = sp['fa0'-'fa7'];
                    #       [ft8-ft11] = sp['ft8'-'ft11'];
                    #       fcsr = sp.fcsr;
                    #       sp += 1;
                    #   else:
                    #       xstatus.FS[1] = 0
                    #
                    li a1, {FS_1}
                    and a0, a0, a1
                    beqz a0, 1f      # → NoPopFLSF

                    FLOAD ft0, ({F_SIZE} * 0)(sp)
                    FLOAD ft1, ({F_SIZE} * 1)(sp)
                    FLOAD ft2, ({F_SIZE} * 2)(sp)
                    FLOAD ft3, ({F_SIZE} * 3)(sp)
                    FLOAD ft4, ({F_SIZE} * 4)(sp)
                    FLOAD ft5, ({F_SIZE} * 5)(sp)
                    FLOAD ft6, ({F_SIZE} * 6)(sp)
                    FLOAD ft7, ({F_SIZE} * 7)(sp)
                    FLOAD fa0, ({F_SIZE} * 8)(sp)
                    FLOAD fa1, ({F_SIZE} * 9)(sp)
                    FLOAD fa2, ({F_SIZE} * 10)(sp)
                    FLOAD fa3, ({F_SIZE} * 11)(sp)
                    FLOAD fa4, ({F_SIZE} * 12)(sp)
                    FLOAD fa5, ({F_SIZE} * 13)(sp)
                    FLOAD fa6, ({F_SIZE} * 14)(sp)
                    FLOAD fa7, ({F_SIZE} * 15)(sp)
                    FLOAD ft8, ({F_SIZE} * 16)(sp)
                    FLOAD ft9, ({F_SIZE} * 17)(sp)
                    FLOAD ft10, ({F_SIZE} * 18)(sp)
                    FLOAD ft11, ({F_SIZE} * 19)(sp)
                    LOAD a0, ({F_SIZE} * 20)(sp)
                    addi sp, sp, {FLSF_SIZE}

                    csrw fcsr, a0

                    j 0f    # → PopFLSFEnd
                1:      # NoPopFLSF
                    csrc " crate::threading::imp::csr::csrexpr!(XSTATUS) ", a1
                0:      # PopFLSFEnd
            "   } else {                                                            "
                    # unused: {F_SIZE} {FLSF_SIZE}
            "   }                                                                   "

                # xstatus.XPP := X (if `PRIVILEGE_LEVEL != U`)
                # xstatus.XPIE := 1 (if `maintain-mpie` is enabled)
                .if {PRIV} == 3
            "       if cfg!(feature = "maintain-pie") {                             "
                        li a0, {MPP_M} | " crate::threading::imp::csr::csrexpr!(XSTATUS_XPIE) "
            "       } else {                                                        "
                        li a0, {MPP_M}
            "       }                                                               "
                    csrs " crate::threading::imp::csr::csrexpr!(XSTATUS) ", a0
                .elseif {PRIV} == 1
            "       if cfg!(feature = "maintain-pie") {                             "
                        li a0, {SPP_S} | " crate::threading::imp::csr::csrexpr!(XSTATUS_XPIE) "
            "       } else {                                                        "
                        li a0, {SPP_S}
            "       }                                                               "
                    csrs " crate::threading::imp::csr::csrexpr!(XSTATUS) ", a0
                .elseif {PRIV} == 0
            "       if cfg!(feature = "maintain-pie") {                             "
                        csrsi " crate::threading::imp::csr::csrexpr!(XSTATUS) ",    "
                            crate::threading::imp::csr::csrexpr!(XSTATUS_XPIE)      "
            "       }                                                               "
                .else
                    .error \"unsupported `PRIVILEGE_LEVEL`\"
                .endif

                # Resume the next task by restoring FLS.X
                #
                #   <[s0-s11, sp] = resumed context>
                #
                #   xepc = sp[16];
                #   [ra, t0-t2, a0-a5] = sp[0..10];
                #   [a6-a7, t3-t6] = sp[10..16];
                #   sp += 17;
                #
                #   pc = xepc;
                #   mode = xstatus.XPP;
                #
                #   <end of procedure>
                #
                LOAD a7, ({X_SIZE} * 16)(sp)
                LOAD ra, ({X_SIZE} * 0)(sp)
                LOAD t0, ({X_SIZE} * 1)(sp)
                LOAD t1, ({X_SIZE} * 2)(sp)
                LOAD t2, ({X_SIZE} * 3)(sp)
                csrw " crate::threading::imp::csr::csrexpr!(XEPC) ", a7
                LOAD a0, ({X_SIZE} * 4)(sp)
                LOAD a1, ({X_SIZE} * 5)(sp)
                LOAD a2, ({X_SIZE} * 6)(sp)
                LOAD a3, ({X_SIZE} * 7)(sp)
                LOAD a4, ({X_SIZE} * 8)(sp)
                LOAD a5, ({X_SIZE} * 9)(sp)
                LOAD a6, ({X_SIZE} * 10)(sp)
                LOAD a7, ({X_SIZE} * 11)(sp)
                LOAD t3, ({X_SIZE} * 12)(sp)
                LOAD t4, ({X_SIZE} * 13)(sp)
                LOAD t5, ({X_SIZE} * 14)(sp)
                LOAD t6, ({X_SIZE} * 15)(sp)
                addi sp, sp, ({X_SIZE} * 17)
                .if {PRIV} == 0
                    uret
                .elseif {PRIV} == 1
                    sret
                .elseif {PRIV} == 3
                    mret
                .else
                    .error \"unsupported `PRIVILEGE_LEVEL`\"
                .endif

            .global {push_second_level_state_and_dispatch}.idle_task
            {push_second_level_state_and_dispatch}.idle_task:
                # The idle task loop. Give it a globoal symbol name to aid
                # debugging.
                #
                #   sp = 0;
                #   xstatus.XIE = 1;
                #   loop:
                #       wfi();
                #
                mv sp, zero
                csrsi " crate::threading::imp::csr::csrexpr!(XSTATUS) ",            "
                    crate::threading::imp::csr::csrexpr!(XSTATUS_XIE)               "
            3:
                wfi
                j 3b
                ",
                push_second_level_state_and_dispatch =
                    sym Self::push_second_level_state_and_dispatch::<Traits>,
                choose_and_get_next_task = sym choose_and_get_next_task::<Traits>,
                get_running_task = sym get_running_task::<Traits>,
                MAIN_STACK = sym MAIN_STACK,
                DISPATCH_PENDING = sym DISPATCH_PENDING,
                MPP_M = const csr::XSTATUS_MPP_M,
                SPP_S = const csr::XSTATUS_SPP_S,
                PRIV = sym <<Traits as PortInstance>::Priv as csr::Num>::value,
                FS_1 = const csr::XSTATUS_FS_1,
                X_SIZE = const X_SIZE,
                F_SIZE = const F_SIZE,
                FLSF_SIZE = const FLSF_SIZE,
                options(noreturn)
            );
        }
    }

    pub unsafe fn exit_and_dispatch<Traits: PortInstance>(
        &'static self,
        _task: &'static TaskCb<Traits>,
    ) -> ! {
        unsafe {
            pp_asm!("
                # XIE := 0
                csrci " crate::threading::imp::csr::csrexpr!(XSTATUS) ",            "
                    crate::threading::imp::csr::csrexpr!(XSTATUS_XIE)               "

                j {push_second_level_state_and_dispatch}.dispatch
                ",
                PRIV = sym <<Traits as PortInstance>::Priv as csr::Num>::value,
                push_second_level_state_and_dispatch =
                    sym Self::push_second_level_state_and_dispatch::<Traits>,
                options(noreturn, nostack),
            );
        }
    }

    #[inline(always)]
    pub unsafe fn enter_cpu_lock<Traits: PortInstance>(&self) {
        Traits::Csr::xstatus_clear_xie();
    }

    #[inline(always)]
    pub unsafe fn try_enter_cpu_lock<Traits: PortInstance>(&self) -> bool {
        (Traits::Csr::xstatus_fetch_clear_xie() & Traits::Csr::XSTATUS_XIE) != 0
    }

    #[inline(always)]
    pub unsafe fn leave_cpu_lock<Traits: PortInstance>(&'static self) {
        Traits::Csr::xstatus_set_xie();
    }

    pub unsafe fn initialize_task_state<Traits: PortInstance>(
        &self,
        task: &'static TaskCb<Traits>,
    ) {
        let stack = task.attr.stack.as_ptr();
        let mut sp = (stack as *mut u8).wrapping_add(stack.len()) as *mut MaybeUninit<usize>;
        // TODO: Enforce minimum stack size

        let preload_all = cfg!(feature = "preload-registers");

        #[inline]
        fn preload_val(i: usize) -> MaybeUninit<usize> {
            match () {
                #[cfg(target_pointer_width = "32")]
                () => MaybeUninit::new(i * 0x01010101),
                #[cfg(target_pointer_width = "64")]
                () => MaybeUninit::new(i * 0x0101010101010101),
                #[cfg(target_pointer_width = "128")]
                () => MaybeUninit::new(i * 0x01010101010101010101010101010101),
            }
        }

        // First-level state (always saved and restored as part of our exception
        // entry/return sequence)
        let first_level = unsafe {
            sp = sp.wrapping_sub(17);
            slice::from_raw_parts_mut(sp, 17)
        };

        // ra: The return address
        first_level[0] =
            MaybeUninit::new(<System<Traits> as traits::KernelBase>::raw_exit_task as usize);
        // t0-t2: Uninitialized
        if preload_all {
            first_level[1] = preload_val(0x05);
            first_level[2] = preload_val(0x06);
            first_level[3] = preload_val(0x07);
        }
        // a0: Parameter to the entry point
        first_level[4] = MaybeUninit::new(task.attr.entry_param as usize);
        // a1-a7: Uninitialized
        if preload_all {
            first_level[5] = preload_val(0x11);
            first_level[6] = preload_val(0x12);
            first_level[7] = preload_val(0x13);
            first_level[8] = preload_val(0x14);
            first_level[9] = preload_val(0x15);
            first_level[10] = preload_val(0x16);
            first_level[11] = preload_val(0x17);
        }
        // t3-t6: Uninitialized
        if preload_all {
            first_level[12] = preload_val(0x28);
            first_level[13] = preload_val(0x29);
            first_level[14] = preload_val(0x30);
            first_level[15] = preload_val(0x31);
        }
        // pc: The entry point
        first_level[16] = MaybeUninit::new(task.attr.entry_point as usize as usize);

        // Second-level state (saved and restored only when we are doing context
        // switching)
        let extra_ctx = unsafe {
            sp = sp.wrapping_sub(12);
            slice::from_raw_parts_mut(sp, 12)
        };

        // SLS.X
        // s0-s12: Uninitialized
        if preload_all {
            extra_ctx[0] = preload_val(0x08);
            extra_ctx[1] = preload_val(0x09);
            extra_ctx[2] = preload_val(0x18);
            extra_ctx[3] = preload_val(0x19);
            extra_ctx[4] = preload_val(0x20);
            extra_ctx[5] = preload_val(0x21);
            extra_ctx[6] = preload_val(0x22);
            extra_ctx[7] = preload_val(0x23);
            extra_ctx[8] = preload_val(0x24);
            extra_ctx[9] = preload_val(0x25);
            extra_ctx[10] = preload_val(0x26);
            extra_ctx[11] = preload_val(0x27);
        }

        // SLS.F is non-existent when `xstatus.FS[1] == 0`

        // SLS.HDR
        if cfg!(target_feature = "f") {
            // xstatus
            //  - FS[1] = 0
            sp = sp.wrapping_sub(1);
            unsafe { *sp = MaybeUninit::new(0) };
        }

        let task_state = &task.port_task_state;
        unsafe { *task_state.sp.get() = sp as _ };
    }

    #[inline(always)]
    pub fn is_cpu_lock_active<Traits: PortInstance>(&self) -> bool {
        (Traits::Csr::xstatus().read() & Traits::Csr::XSTATUS_XIE) == 0
    }

    pub fn is_task_context<Traits: PortInstance>(&self) -> bool {
        unsafe { INTERRUPT_NESTING < 0 }
    }

    pub fn set_interrupt_line_priority<Traits: PortInstance>(
        &'static self,
        num: InterruptNum,
        priority: InterruptPriority,
    ) -> Result<(), SetInterruptLinePriorityError> {
        if num < INTERRUPT_PLATFORM_START {
            Err(SetInterruptLinePriorityError::BadParam)
        } else {
            // Safety: We are delegating the call in the intended way
            unsafe { <Traits as InterruptController>::set_interrupt_line_priority(num, priority) }
        }
    }

    #[inline]
    pub fn enable_interrupt_line<Traits: PortInstance>(
        &'static self,
        num: InterruptNum,
    ) -> Result<(), EnableInterruptLineError> {
        if num < INTERRUPT_PLATFORM_START {
            // Enabling or disabling local interrupt lines is not supported
            Err(EnableInterruptLineError::BadParam)
        } else {
            // Safety: We are delegating the call in the intended way
            unsafe { <Traits as InterruptController>::enable_interrupt_line(num) }
        }
    }

    #[inline]
    pub fn disable_interrupt_line<Traits: PortInstance>(
        &self,
        num: InterruptNum,
    ) -> Result<(), EnableInterruptLineError> {
        if num < INTERRUPT_PLATFORM_START {
            // Enabling or disabling local interrupt lines is not supported
            Err(EnableInterruptLineError::BadParam)
        } else {
            // Safety: We are delegating the call in the intended way
            unsafe { <Traits as InterruptController>::disable_interrupt_line(num) }
        }
    }

    #[inline]
    pub fn pend_interrupt_line<Traits: PortInstance>(
        &'static self,
        num: InterruptNum,
    ) -> Result<(), PendInterruptLineError> {
        if num == INTERRUPT_SOFTWARE {
            Traits::Csr::xip().set(Traits::Csr::XIP_XSIP);
            Ok(())
        } else if num < INTERRUPT_PLATFORM_START {
            Err(PendInterruptLineError::BadParam)
        } else {
            // Safety: We are delegating the call in the intended way
            unsafe { <Traits as InterruptController>::pend_interrupt_line(num) }
        }
    }

    #[inline]
    pub fn clear_interrupt_line<Traits: PortInstance>(
        &self,
        num: InterruptNum,
    ) -> Result<(), ClearInterruptLineError> {
        if num == INTERRUPT_SOFTWARE {
            Traits::Csr::xip().clear(Traits::Csr::XIP_XSIP);
            Ok(())
        } else if num < INTERRUPT_PLATFORM_START {
            Err(ClearInterruptLineError::BadParam)
        } else {
            // Safety: We are delegating the call in the intended way
            unsafe { <Traits as InterruptController>::clear_interrupt_line(num) }
        }
    }

    #[inline]
    pub fn is_interrupt_line_pending<Traits: PortInstance>(
        &self,
        num: InterruptNum,
    ) -> Result<bool, QueryInterruptLineError> {
        if num < INTERRUPT_PLATFORM_START {
            Ok((Traits::Csr::xip().read() & (Traits::Csr::XIP_XSIP << (num * 4))) != 0)
        } else {
            // Safety: We are delegating the call in the intended way
            unsafe { <Traits as InterruptController>::is_interrupt_line_pending(num) }
        }
    }

    #[inline]
    pub unsafe fn enable_external_interrupts<Traits: PortInstance>(&self) {
        Traits::Csr::xie().set(Traits::Csr::XIE_XEIE);
    }

    #[inline]
    pub unsafe fn disable_external_interrupts<Traits: PortInstance>(&self) {
        Traits::Csr::xie().clear(Traits::Csr::XIE_XEIE);
    }

    /// Implements [`crate::EntryPoint::exception_handler`].
    #[naked]
    pub unsafe extern "C" fn exception_handler<Traits: PortInstance>() -> ! {
        const FRAME_SIZE: usize = if cfg!(target_feature = "f") {
            // [background_sp, xstatus]
            X_SIZE * 2
        } else {
            // [background_sp]
            X_SIZE
        };

        // FIXME: Add `#[repr(align(4))]` to this function. `.align 2` in `asm!`
        //        actually doesn't work as intended. This attribute is being
        //        implemented by the following PR:
        //        <https://github.com/rust-lang/rust/pull/81234>

        unsafe {
            pp_asm!("
            "   crate::threading::imp::asm_inc::define_load_store!()              "
            "   crate::threading::imp::asm_inc::define_fload_fstore!()              "

                # Align the handler to a 4-byte boundary
                .align 2

                # Skip the stacking of FLS if the background context is the idle
                # task.
                #
                #   <[a0-a7, t0-t6, s0-s11, sp] = background context state,
                #    background context ∈ [task, idle task, interrupt]>
                #   if sp == 0:
                #       xstatus_part = 0;
                #       <background context ∈ [idle task], a2 == xstatus_part>
                #       INTERRUPT_NESTING += 1;
                #       goto SwitchToMainStack;
                #
                beqz sp, 3f     # → EntryFromIdleTask

                # Push FLS.X to the background context's stack
                #
                #   <[a0-a7, t0-t6, s0-s11, sp] = background context state,
                #    background context ∈ [task, interrupt], sp != 0>
                #
                #   sp -= 17;
                #   sp[0..10] = [ra, t0-t2, a0-a5];
                #   sp[10..16] = [a6-a7, t3-t6];
                #   sp[16] = xepc
                #
                #   let background_sp = sp;
                #   let background_flsx = sp;
                #   <[s0-s11] = background context state, sp != 0>
                #
                addi sp, sp, (-{X_SIZE} * 17)
                STORE ra, ({X_SIZE} * 0)(sp)
                STORE t0, ({X_SIZE} * 1)(sp)
                STORE t1, ({X_SIZE} * 2)(sp)
                STORE t2, ({X_SIZE} * 3)(sp)
                STORE a0, ({X_SIZE} * 4)(sp)
                STORE a1, ({X_SIZE} * 5)(sp)
                                                # Increment the nesting count.
                                                #
                                                #   <INTERRUPT_NESTING ≥ -1>
                                                #   INTERRUPT_NESTING += 1;
                                                #   <INTERRUPT_NESTING ≥ 0>
                                                #
                                                la a1, {INTERRUPT_NESTING}
                                                lw a0, (a1)
                STORE a2, ({X_SIZE} * 6)(sp)
                csrr a2, " crate::threading::imp::csr::csrexpr!(XEPC) "
                STORE a3, ({X_SIZE} * 7)(sp)
                STORE a4, ({X_SIZE} * 8)(sp)
                STORE a5, ({X_SIZE} * 9)(sp)
                STORE a6, ({X_SIZE} * 10)(sp)
                STORE a7, ({X_SIZE} * 11)(sp)
                STORE t3, ({X_SIZE} * 12)(sp)
                STORE t4, ({X_SIZE} * 13)(sp)
                STORE t5, ({X_SIZE} * 14)(sp)
                STORE t6, ({X_SIZE} * 15)(sp)
                STORE a2, ({X_SIZE} * 16)(sp)
            "   if cfg!(target_feature = "f") {                                     "
                    csrr a2, " crate::threading::imp::csr::csrexpr!(XSTATUS) "
            "   }                                                                   "
                                                addi a0, a0, 1
                                                sw a0, (a1)

            "   if cfg!(target_feature = "f") {                                     "
                    # If FP registers are in use, push FLS.F to the background
                    # context's stack. Clear `xstatus.FS[1]` to indicate that
                    # FP registers are not in use in the current invocation of
                    # the trap handler (it'll be set again on first use).
                    #
                    #   <a2 = xstatus_part>
                    #   if xstatus_part.FS[1] != 0:
                    #       sp: *mut FlsF;
                    #       sp -= 1;
                    #       sp['ft0'-'ft7'] = [ft0-ft7];
                    #       sp['fa0'-'fa7'] = [fa0-fa7];
                    #       sp['ft8'-'ft11'] = [ft8-ft11];
                    #       sp.fcsr = fcsr;
                    #       xstatus.FS[1] = 0;
                    #
                    #   let background_sp = sp;
                    #   <a2 = xstatus_part>
                    #
                    li a1, {FS_1}
                    and a1, a1, a2
                    beqz a1, 0f      # → PushFLSFEnd

                    csrc " crate::threading::imp::csr::csrexpr!(XSTATUS) ", a1
                    csrr a1, fcsr

                    addi sp, sp, -{FLSF_SIZE}
                    FSTORE ft0, ({F_SIZE} * 0)(sp)
                    FSTORE ft1, ({F_SIZE} * 1)(sp)
                    FSTORE ft2, ({F_SIZE} * 2)(sp)
                    FSTORE ft3, ({F_SIZE} * 3)(sp)
                    FSTORE ft4, ({F_SIZE} * 4)(sp)
                    FSTORE ft5, ({F_SIZE} * 5)(sp)
                    FSTORE ft6, ({F_SIZE} * 6)(sp)
                    FSTORE ft7, ({F_SIZE} * 7)(sp)
                    FSTORE fa0, ({F_SIZE} * 8)(sp)
                    FSTORE fa1, ({F_SIZE} * 9)(sp)
                    FSTORE fa2, ({F_SIZE} * 10)(sp)
                    FSTORE fa3, ({F_SIZE} * 11)(sp)
                    FSTORE fa4, ({F_SIZE} * 12)(sp)
                    FSTORE fa5, ({F_SIZE} * 13)(sp)
                    FSTORE fa6, ({F_SIZE} * 14)(sp)
                    FSTORE fa7, ({F_SIZE} * 15)(sp)
                    FSTORE ft8, ({F_SIZE} * 16)(sp)
                    FSTORE ft9, ({F_SIZE} * 17)(sp)
                    FSTORE ft10, ({F_SIZE} * 18)(sp)
                    FSTORE ft11, ({F_SIZE} * 19)(sp)
                    STORE a1, ({F_SIZE} * 20)(sp)
                0:      # PushFLSFEnd
            "   } else {                                                            "
                    # unused: {F_SIZE} {FS_1} {FLSF_SIZE}
            "   }                                                                   "

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
                bnez a0, 0f     # → RealignStack

            4:      # SwitchToMainStack
                # If the background context is a task context, we should switch
                # to `MAIN_STACK`. Meanwhile, push the original `sp` to
                # `MAIN_STACK`.
                #
                #   <INTERRUPT_NESTING == 0, background context ∈ [task, idle task],
                #    a2 == xstatus_part>
                #   *(MAIN_STACK - ceil(FRAME_SIZE, 16)) = sp;
                #   sp = MAIN_STACK - ceil(FRAME_SIZE, 16);
                #   <sp[0] == background_sp, sp & 15 == 0, sp != 0,
                #    a0 == background_sp, a2 == xstatus_part>
                #
                mv a0, sp
                LOAD sp, ({MAIN_STACK})
                addi sp, sp, -(({FRAME_SIZE} + 15) / 16 * 16)
                STORE a0, (sp)

                j 1f            # → RealignStackEnd

            0:       # RealignStack
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
                #   <INTERRUPT_NESTING > 0, background context ∈ [interrupt],
                #    a2 == xstatus_part>
                #   *((sp - FRAME_SIZE) & !15) = sp
                #   sp = (sp - FRAME_SIZE) & !15
                #   <sp[0] == background_sp, sp & 15 == 0, sp != 0,
                #    a0 == background_sp, a2 == xstatus_part>
                #
                mv a0, sp
                addi sp, sp, -{FRAME_SIZE}
                andi sp, sp, -16
                STORE a0, (sp)

            1:      # RealignStackEnd
            "   if cfg!(target_feature = "f") {                                     "
                    # Save `xstatus_part`.
                    STORE a2, {X_SIZE}(sp)
            "   }                                                                   "

                # Check `xcause.Interrurpt`.
                csrr a1, " crate::threading::imp::csr::csrexpr!(XCAUSE) "
                srli a3, a1, 31
                beqz a3, 1f

                # If the cause is an interrupt, call `handle_interrupt`
                #
                #   handle_interrupt();
                #
                call {handle_interrupt}

                # Invalidate any reservation held by this hart (this will cause
                # a subsequent Store-Conditional to fail). Don't do this for a
                # software trap because a software trap can be used to emulate
                # an SC/LR instruction.
                #
                # > Trap handlers should explicitly clear the reservation if
                # > required (e.g., by using a dummy SC) before executing the
                # > xRET.
            "   if cfg!(feature = "emulate-lr-sc")  {                               "
                    STORE x0, ({RESERVATION_ADDR_VALUE}), a1
            "   } else {                                                            "
                    # unused: {RESERVATION_ADDR_VALUE}
                    addi a1, sp, -{X_SIZE}
                    sc.w x0, x0, (a1)
            "   }                                                                   "

                j 2f
            1:
                # If the cause is a software trap, call `handle_exception`
            "   if cfg!(target_feature = "f") {                                     "
                    #
                    #   <a0 == background_sp, a1 == xcause, a2 = xstatus_part>
                    #   if xstatus_part.FS[1]:
                    #       a0 += FLSF_SIZE;
                    #
                    slli a2, a2, {X_SIZE} * 8 - 1 - {FS_1_SHIFT}
                    bgez a2, 1f     # → NoFLSF
                    addi a0, a0, {FLSF_SIZE}
                1:      # NoFLSF
            "   } else {                                                            "
                    # unused: {FS_1_SHIFT}
            "   }                                                                   "
                #
                #   <a0 == background_flsx, a1 == xcause>
                #   handle_exception(a0, a1);
                #
                call {handle_exception}
            2:

                                            # Decrement the nesting count.
                                            #
                                            #   <INTERRUPT_NESTING ≥ 0>
                                            #   INTERRUPT_NESTING -= 1;
                                            #   <INTERRUPT_NESTING ≥ -1>
                                            #
                                            la a2, {INTERRUPT_NESTING}
                                            lw a1, (a2)

            "   if cfg!(target_feature = "f") {                                     "
                    # Restore `xstatus_part`
                    LOAD a0, {X_SIZE}(sp)
            "   }                                                                   "

                # Restore `background_sp`
                LOAD sp, (sp)

                                            addi a1, a1, -1
                                            sw a1, (a2)

                # Are we returning to an interrupt context?
                #
                # If we are returning to an outer interrupt handler, finding the
                # next task to dispatch is unnecessary, so we can jump straight
                # to `pop_first_level_state`.
                #
                #   <INTERRUPT_NESTING ≥ 0>
                #   if INTERRUPT_NESTING > 0:
                #       goto pop_first_level_state;
                #
                bgez a1, 2f

                # Return to the task context by restoring the first-level and
                # second-level state of the next task.
                tail {push_second_level_state_and_dispatch}

            2:
                tail {push_second_level_state_and_dispatch}.pop_first_level_state

            3:      # EntryFromIdleTask
                # Increment the nesting count.
                #
                #   <INTERRUPT_NESTING == -1, background context ∈ [idle task]>
                #   INTERRUPT_NESTING += 1;
                #   <INTERRUPT_NESTING == 0>
                #
                sw x0, ({INTERRUPT_NESTING}), a1
                mv a2, x0
                j 4b        # → SwitchToMainStack
                ",
                handle_interrupt = sym Self::handle_interrupt::<Traits>,
                handle_exception = sym instemu::handle_exception,
                push_second_level_state_and_dispatch =
                    sym Self::push_second_level_state_and_dispatch::<Traits>,
                INTERRUPT_NESTING = sym INTERRUPT_NESTING,
                RESERVATION_ADDR_VALUE = sym instemu::RESERVATION_ADDR_VALUE,
                MAIN_STACK = sym MAIN_STACK,
                X_SIZE = const X_SIZE,
                F_SIZE = const F_SIZE,
                FLSF_SIZE = const FLSF_SIZE,
                FRAME_SIZE = const FRAME_SIZE,
                PRIV = sym <<Traits as PortInstance>::Priv as csr::Num>::value,
                FS_1 = const csr::XSTATUS_FS_1,
                FS_1_SHIFT = const csr::XSTATUS_FS_1.trailing_zeros(),
                options(noreturn)
            );
        }
    }

    unsafe fn handle_interrupt<Traits: PortInstance>() {
        let all_local_interrupts = [0, Traits::Csr::XIE_XSIE]
            [Traits::USE_INTERRUPT_SOFTWARE as usize]
            | [0, Traits::Csr::XIE_XTIE][Traits::USE_INTERRUPT_TIMER as usize]
            | [0, Traits::Csr::XIE_XEIE][Traits::USE_INTERRUPT_EXTERNAL as usize];

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
        //  let mut found_bg_exc_pri;        // Represented by `xie_pending`
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
        let old_mie = Traits::Csr::xie().fetch_clear(all_local_interrupts);
        let mut xie_pending = 0;

        // Re-enable interrupts globally.
        Traits::Csr::xstatus_set_xie();

        let mut xip = Traits::Csr::xip().read();

        // Check the pending flags and call the respective handlers in the
        // descending order of priority.
        if Traits::USE_INTERRUPT_EXTERNAL && (old_mie & Traits::Csr::XIE_XEIE) != 0 {
            // Safety: `USE_INTERRUPT_EXTERNAL == true`
            let handler = Traits::INTERRUPT_EXTERNAL_HANDLER
                .unwrap_or_else(|| unsafe { unreachable_unchecked() });

            while (xip & Traits::Csr::XIP_XEIP) != 0 {
                // Safety: The first-level interrupt handler is allowed to call
                //         a second-level interrupt handler
                unsafe { handler() };

                xip = Traits::Csr::xip().read();
            }

            xie_pending = Traits::Csr::XIE_XEIE;
        }

        if Traits::USE_INTERRUPT_SOFTWARE && (old_mie & Traits::Csr::XIE_XSIE) != 0 {
            // Safety: `USE_INTERRUPT_SOFTWARE == true`
            let handler = Traits::INTERRUPT_SOFTWARE_HANDLER
                .unwrap_or_else(|| unsafe { unreachable_unchecked() });

            if Traits::USE_INTERRUPT_EXTERNAL {
                debug_assert_eq!(xie_pending, Traits::Csr::XIE_XEIE);
                Traits::Csr::xie().set(Traits::Csr::XIE_XEIE);
            } else {
                debug_assert_eq!(xie_pending, 0);
            }

            while (xip & Traits::Csr::XIP_XSIP) != 0 {
                // Safety: The first-level interrupt handler is allowed to call
                //         a second-level interrupt handler
                unsafe { handler() };

                xip = Traits::Csr::xip().read();
            }

            xie_pending = Traits::Csr::XIE_XSIE;
        }

        if Traits::USE_INTERRUPT_TIMER && (old_mie & Traits::Csr::XIE_XTIE) != 0 {
            // Safety: `USE_INTERRUPT_TIMER == true`
            let handler = Traits::INTERRUPT_TIMER_HANDLER
                .unwrap_or_else(|| unsafe { unreachable_unchecked() });

            if Traits::USE_INTERRUPT_SOFTWARE {
                debug_assert_eq!(xie_pending, Traits::Csr::XIE_XSIE);
                Traits::Csr::xie().set(Traits::Csr::XIE_XSIE);
            } else if Traits::USE_INTERRUPT_EXTERNAL {
                debug_assert_eq!(xie_pending, Traits::Csr::XIE_XEIE);
                Traits::Csr::xie().set(Traits::Csr::XIE_XEIE);
            } else {
                debug_assert_eq!(xie_pending, 0);
            }

            while (xip & Traits::Csr::XIP_XTIP) != 0 {
                // Safety: The first-level interrupt handler is allowed to call
                //         a second-level interrupt handler
                unsafe { handler() };

                xip = Traits::Csr::xip().read();
            }

            xie_pending = Traits::Csr::XIE_XTIE;
        }

        // Disable interrupts globally before returning.
        Traits::Csr::xstatus_clear_xie();

        debug_assert_ne!(xie_pending, 0);
        Traits::Csr::xie().set(xie_pending);
    }
}

/// Used by `use_port!`
pub const fn validate<Traits: PortInstance>() {}
