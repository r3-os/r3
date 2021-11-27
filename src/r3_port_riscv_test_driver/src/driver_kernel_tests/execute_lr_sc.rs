//! Executes LR and SC instructions with various parameters. This test will
//! exercise the emulation code (`emulate-lr-sc`) on some targets.
use core::{marker::PhantomData, mem::MaybeUninit, ptr::addr_of_mut};
use r3::kernel::{prelude::*, traits, Cfg, StartupHook, Task};
use r3_portkit::pptext::pp_asm;
use r3_test_suite::kernel_tests::Driver;

pub struct App<System> {
    _phantom: PhantomData<System>,
}

impl<System: traits::KernelBase> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System> + ~const traits::CfgTask,
    {
        StartupHook::build()
            .start(startup_hook_body::<System, D>)
            .finish(b);

        Task::build()
            .start(task_body::<System, D>)
            .priority(0)
            .active(true)
            .finish(b);

        App {
            _phantom: PhantomData,
        }
    }
}

fn startup_hook_body<System: traits::KernelBase, D: Driver<App<System>>>(_: usize) {
    log::debug!("calling do_test from a startup hook");
    unsafe { do_test::<System>() };
}

fn task_body<System: traits::KernelBase, D: Driver<App<System>>>(_: usize) {
    log::debug!("calling do_test from a task");
    unsafe {
        System::acquire_cpu_lock().unwrap();
        do_test::<System>();
        System::release_cpu_lock().unwrap();
    }
    D::success();
}

#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(C)]
struct St {
    x: [*mut u8; 32],
}

unsafe impl Send for St {}
unsafe impl Sync for St {}

static mut VAR: isize = 0;
static mut ST1: MaybeUninit<St> = MaybeUninit::uninit();
static mut ST2: MaybeUninit<St> = MaybeUninit::uninit();

// Creating a `static` item referencing to another `static mut` item is tricky.
// Therefore we circumvent the restriction by `global_asm!`.
extern "C" {
    static execute_lr_sc_init_st: St;
}

global_asm!(
    ".section .bss
    .align 4
    execute_lr_sc_alt_stack:
        .zero 1024
    execute_lr_sc_alt_stack_top:"
);

#[cfg(target_pointer_width = "32")]
global_asm!(
    ".section .rodata
    execute_lr_sc_init_st:
        .word 0x00000000
        .word 0x01010101
        .word execute_lr_sc_alt_stack_top - 16
        .word 0x03030303
        .word 0x04040404
        .word 0x05050505
        .word 0x06060606
        .word 0x07070707
        .word 0x08080808
        .word 0x09090909
        .word 0x10101010
        .word 0x11111111
        .word 0x12121212
        .word 0x13131313
        .word 0x14141414
        .word 0x15151515
        .word 0x16161616
        .word 0x17171717
        .word 0x18181818
        .word 0x19191919
        .word 0x20202020
        .word 0x21212121
        .word 0x22222222
        .word 0x23232323
        .word 0x24242424
        .word 0x25252525
        .word 0x26262626
        .word 0x27272727
        .word 0x28282828
        .word 0x29292929
        .word 0x30303030
        .word 0x31313131"
);

#[cfg(target_pointer_width = "64")]
global_asm!(
    ".section .rodata
    execute_lr_sc_init_st:
        .quad 0x0000000000000000
        .quad 0x0101010101010101
        .quad execute_lr_sc_alt_stack_top - 16
        .quad 0x0303030303030303
        .quad 0x0404040404040404
        .quad 0x0505050505050505
        .quad 0x0606060606060606
        .quad 0x0707070707070707
        .quad 0x0808080808080808
        .quad 0x0909090909090909
        .quad 0x1010101010101010
        .quad 0x1111111111111111
        .quad 0x1212121212121212
        .quad 0x1313131313131313
        .quad 0x1414141414141414
        .quad 0x1515151515151515
        .quad 0x1616161616161616
        .quad 0x1717171717171717
        .quad 0x1818181818181818
        .quad 0x1919191919191919
        .quad 0x2020202020202020
        .quad 0x2121212121212121
        .quad 0x2222222222222222
        .quad 0x2323232323232323
        .quad 0x2424242424242424
        .quad 0x2525252525252525
        .quad 0x2626262626262626
        .quad 0x2727272727272727
        .quad 0x2828282828282828
        .quad 0x2929292929292929
        .quad 0x3030303030303030
        .quad 0x3131313131313131"
);

#[cfg(target_pointer_width = "128")]
global_asm!(
    ".section .rodata
    execute_lr_sc_init_st:
        .octa 0x00000000000000000000000000000000
        .octa 0x01010101010101010101010101010101
        .octa execute_lr_sc_alt_stack_top - 16
        .octa 0x03030303030303030303030303030303
        .octa 0x04040404040404040404040404040404
        .octa 0x05050505050505050505050505050505
        .octa 0x06060606060606060606060606060606
        .octa 0x07070707070707070707070707070707
        .octa 0x08080808080808080808080808080808
        .octa 0x09090909090909090909090909090909
        .octa 0x10101010101010101010101010101010
        .octa 0x11111111111111111111111111111111
        .octa 0x12121212121212121212121212121212
        .octa 0x13131313131313131313131313131313
        .octa 0x14141414141414141414141414141414
        .octa 0x15151515151515151515151515151515
        .octa 0x16161616161616161616161616161616
        .octa 0x17171717171717171717171717171717
        .octa 0x18181818181818181818181818181818
        .octa 0x19191919191919191919191919191919
        .octa 0x20202020202020202020202020202020
        .octa 0x21212121212121212121212121212121
        .octa 0x22222222222222222222222222222222
        .octa 0x23232323232323232323232323232323
        .octa 0x24242424242424242424242424242424
        .octa 0x25252525252525252525252525252525
        .octa 0x26262626262626262626262626262626
        .octa 0x27272727272727272727272727272727
        .octa 0x28282828282828282828282828282828
        .octa 0x29292929292929292929292929292929
        .octa 0x30303030303030303030303030303030
        .octa 0x31313131313131313131313131313131"
);

/// `XLEN / 8`
const X_SIZE: usize = core::mem::size_of::<usize>();

#[cfg(not(target_feature = "a"))]
unsafe fn do_test<System: traits::KernelBase>() {
    log::warn!("The 'A' extension is disabled, skipping the test");
}

/// The core of this test case.
///
/// # Safety
///
/// Interrupts must be disabled.
#[cfg(target_feature = "a")]
unsafe fn do_test<System: traits::KernelBase>() {
    macro exec($code:literal, |$st:ident| $behavior:expr) {
        log::trace!("{}", $code);
        unsafe {
            pp_asm!("
            "   r3_port_riscv::threading::imp::asm_inc::define_load_store!() "
                # ST2 = current_state();
                call {save_st1}
                la a0, {ST2}
                la a1, {ST1}
                call {copy_st}

                # set_current_state_including_ra(INIT_ST);
                la a0, {INIT_ST}
                call {restore_st}
                LOAD ra, {INIT_ST} + 1 * {X_SIZE}

                # The test code might trash any X registers. `sp` should still
                # be a valid stack pointer after executing the code.
            "   $code                                                               "

                # ST1 = current_state_including_ra();
                STORE ra, (sp)
                call {save_st1}

                la a0, {ST1}
                LOAD ra, (sp)
                STORE ra, (1 * {X_SIZE})(a0)

                # set_current_state(ST2);
                la a0, {ST2}
                call {restore_st}
                ",
                VAR = sym VAR,
                ST1 = sym ST1,
                ST2 = sym ST2,
                X_SIZE = const X_SIZE,
                INIT_ST = sym execute_lr_sc_init_st,
                save_st1 = sym save_st1,
                restore_st = sym restore_st,
                copy_st = sym copy_st,
                out("ra") _,
            );

            // Simulate the intended behavior
            {
                let $st = &mut *ST2.as_mut_ptr();
                *$st = execute_lr_sc_init_st;
                $behavior;
            }

            posttest($code);
        }
    }

    #[inline(never)]
    unsafe fn posttest(code: &str) {
        let got = unsafe { &*ST1.as_ptr() };
        let expected = unsafe { &*ST2.as_ptr() };
        assert_eq!(
            *got, *expected,
            "reached an incorrect final state after executing '{}'",
            code
        );
    }

    const VAR_SEXT: *mut u8 = 0x87654321u32 as i32 as isize as _;
    unsafe { VAR = 0x87654321u32 as usize as isize };

    // `lr.w _, (x6)`
    // ------------------------------------------------------------------
    exec!("la x6, {VAR}; lr.w x0, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
    });
    exec!("la x6, {VAR}; lr.w x1, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[1] = VAR_SEXT;
    });
    // `lr.w sp, (_)` is not supported by this test harness nor
    // `emulate-lr-sc`.
    exec!("la x6, {VAR}; lr.w x3, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[3] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x4, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[4] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x5, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[5] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x6, (x6)", |st| {
        st.x[6] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x7, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[7] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x8, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[8] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x9, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[9] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x10, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[10] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x11, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[11] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x12, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[12] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x13, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[13] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x14, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[14] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x15, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[15] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x16, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[16] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x17, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[17] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x18, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[18] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x19, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[19] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x20, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[20] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x21, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[21] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x22, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[22] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x23, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[23] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x24, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[24] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x25, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[25] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x26, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[26] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x27, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[27] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x28, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[28] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x29, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[29] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x30, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[30] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x31, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[31] = VAR_SEXT;
    });

    // `lr.w x6, (_)`
    // ------------------------------------------------------------------

    // `lr.w _, (x0)` will never succeed unless there's valid data at `0`

    exec!("la x1, {VAR}; lr.w x6, (x1)", |st| {
        st.x[1] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });

    // The emulation code uses the current thread's stack, so `sp` must be
    // really a stack pointer when executing `lr.w`.
    exec!(
        "addi x2, x2, -16
        lw x6, {VAR}; sw x6, (x2); li x6, 0
        lr.w x6, (x2)
        addi x2, x2, 16",
        |st| {
            st.x[6] = VAR_SEXT;
        }
    );

    exec!("la x3, {VAR}; lr.w x6, (x3)", |st| {
        st.x[3] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x4, {VAR}; lr.w x6, (x4)", |st| {
        st.x[4] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x5, {VAR}; lr.w x6, (x5)", |st| {
        st.x[5] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x6, {VAR}; lr.w x7, (x6)", |st| {
        st.x[6] = addr_of_mut!(VAR) as _;
        st.x[7] = VAR_SEXT;
    });
    exec!("la x7, {VAR}; lr.w x6, (x7)", |st| {
        st.x[7] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x8, {VAR}; lr.w x6, (x8)", |st| {
        st.x[8] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x9, {VAR}; lr.w x6, (x9)", |st| {
        st.x[9] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x10, {VAR}; lr.w x6, (x10)", |st| {
        st.x[10] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x11, {VAR}; lr.w x6, (x11)", |st| {
        st.x[11] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x12, {VAR}; lr.w x6, (x12)", |st| {
        st.x[12] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x13, {VAR}; lr.w x6, (x13)", |st| {
        st.x[13] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x14, {VAR}; lr.w x6, (x14)", |st| {
        st.x[14] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x15, {VAR}; lr.w x6, (x15)", |st| {
        st.x[15] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x16, {VAR}; lr.w x6, (x16)", |st| {
        st.x[16] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x17, {VAR}; lr.w x6, (x17)", |st| {
        st.x[17] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x18, {VAR}; lr.w x6, (x18)", |st| {
        st.x[18] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x19, {VAR}; lr.w x6, (x19)", |st| {
        st.x[19] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x20, {VAR}; lr.w x6, (x20)", |st| {
        st.x[20] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x21, {VAR}; lr.w x6, (x21)", |st| {
        st.x[21] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x22, {VAR}; lr.w x6, (x22)", |st| {
        st.x[22] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x23, {VAR}; lr.w x6, (x23)", |st| {
        st.x[23] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x24, {VAR}; lr.w x6, (x24)", |st| {
        st.x[24] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x25, {VAR}; lr.w x6, (x25)", |st| {
        st.x[25] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x26, {VAR}; lr.w x6, (x26)", |st| {
        st.x[26] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x27, {VAR}; lr.w x6, (x27)", |st| {
        st.x[27] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x28, {VAR}; lr.w x6, (x28)", |st| {
        st.x[28] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x29, {VAR}; lr.w x6, (x29)", |st| {
        st.x[29] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x30, {VAR}; lr.w x6, (x30)", |st| {
        st.x[30] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });
    exec!("la x31, {VAR}; lr.w x6, (x31)", |st| {
        st.x[31] = addr_of_mut!(VAR) as _;
        st.x[6] = VAR_SEXT;
    });

    // `sc.w`
    // ------------------------------------------------------------------
    exec!(
        "la x7, {VAR}
        lr.w x6, (x7)
        addi x6, x6, 1
        sc.w x8, x6, (x7)",
        |st| {
            st.x[7] = addr_of_mut!(VAR) as _;
            st.x[6] = VAR_SEXT.wrapping_add(1);
            st.x[8] = 0 as _; // success
        }
    );

    exec!(
        "la x7, {VAR}
        lr.w x6, (x7)
        addi x6, x6, -1
        sw x6, (x7)
        sc.w x8, x0, (x7)
        snez x8, x8",
        |st| {
            st.x[7] = addr_of_mut!(VAR) as _;
            st.x[6] = VAR_SEXT;
            st.x[8] = 1 as _; // non-zero result value, meaning failure
        }
    );

    exec!(
        "la x7, {VAR}
        lr.w x6, (x7)
        sc.w x0, x0, (sp)    # clear the reservation on `VAR`
        sc.w x8, x6, (x7)
        snez x8, x8",
        |st| {
            st.x[7] = addr_of_mut!(VAR) as _;
            st.x[6] = VAR_SEXT;
            st.x[8] = 1 as _; // non-zero result value, meaning failure
        }
    );
}

/// Copy `src` to `dst`.
unsafe extern "C" fn copy_st(dst: *mut St, src: *const St) {
    unsafe { *dst = *src };
}

/// Save to `ST1` all X registers except for `ra` (`x1`). The stack pointer
/// must be valid. All registers are preserved during a call to this function.
#[naked]
extern "C" fn save_st1() {
    unsafe {
        pp_asm!("
        "   r3_port_riscv::threading::imp::asm_inc::define_load_store!() "
            # Save `ALL_X - [x0, x1, x2, x6]`
            addi x2, x2, -16
            STORE x6, (x2)
            la x6, {ST1}
            STORE x3, (3 * {X_SIZE})(x6)
            STORE x4, (4 * {X_SIZE})(x6)
            STORE x5, (5 * {X_SIZE})(x6)
            STORE x7, (7 * {X_SIZE})(x6)
            STORE x8, (8 * {X_SIZE})(x6)
            STORE x9, (9 * {X_SIZE})(x6)
            STORE x10, (10 * {X_SIZE})(x6)
            STORE x11, (11 * {X_SIZE})(x6)
            STORE x12, (12 * {X_SIZE})(x6)
            STORE x13, (13 * {X_SIZE})(x6)
            STORE x14, (14 * {X_SIZE})(x6)
            STORE x15, (15 * {X_SIZE})(x6)
            STORE x16, (16 * {X_SIZE})(x6)
            STORE x17, (17 * {X_SIZE})(x6)
            STORE x18, (18 * {X_SIZE})(x6)
            STORE x19, (19 * {X_SIZE})(x6)
            STORE x20, (20 * {X_SIZE})(x6)
            STORE x21, (21 * {X_SIZE})(x6)
            STORE x22, (22 * {X_SIZE})(x6)
            STORE x23, (23 * {X_SIZE})(x6)
            STORE x24, (24 * {X_SIZE})(x6)
            STORE x25, (25 * {X_SIZE})(x6)
            STORE x26, (26 * {X_SIZE})(x6)
            STORE x27, (27 * {X_SIZE})(x6)
            STORE x28, (28 * {X_SIZE})(x6)
            STORE x29, (29 * {X_SIZE})(x6)
            STORE x30, (30 * {X_SIZE})(x6)
            STORE x31, (31 * {X_SIZE})(x6)

            # Save `[x2, x6]`
            mv x7, x6
            LOAD x6, (x2)
            addi x2, x2, 16
            STORE x2, (2 * {X_SIZE})(x7)
            STORE x6, (6 * {X_SIZE})(x7)

            # Restore `x7`
            LOAD x7, (7 * {X_SIZE})(x7)
            ret
        ",
            ST1 = sym ST1,
            X_SIZE = const X_SIZE,
            options(noreturn),
        );
    }
}

/// Restore from `a0` all X registers except for `ra` (`x1`).
#[naked]
unsafe extern "C" fn restore_st(_a0: *const St) {
    unsafe {
        pp_asm!("
        "   r3_port_riscv::threading::imp::asm_inc::define_load_store!() "
            # Restor `ALL_X - [x0, x1, x10]`
            LOAD x2, (2 * {X_SIZE})(x10)
            LOAD x3, (3 * {X_SIZE})(x10)
            LOAD x4, (4 * {X_SIZE})(x10)
            LOAD x5, (5 * {X_SIZE})(x10)
            LOAD x6, (6 * {X_SIZE})(x10)
            LOAD x7, (7 * {X_SIZE})(x10)
            LOAD x8, (8 * {X_SIZE})(x10)
            LOAD x9, (9 * {X_SIZE})(x10)
            LOAD x11, (11 * {X_SIZE})(x10)
            LOAD x12, (12 * {X_SIZE})(x10)
            LOAD x13, (13 * {X_SIZE})(x10)
            LOAD x14, (14 * {X_SIZE})(x10)
            LOAD x15, (15 * {X_SIZE})(x10)
            LOAD x16, (16 * {X_SIZE})(x10)
            LOAD x17, (17 * {X_SIZE})(x10)
            LOAD x18, (18 * {X_SIZE})(x10)
            LOAD x19, (19 * {X_SIZE})(x10)
            LOAD x20, (20 * {X_SIZE})(x10)
            LOAD x21, (21 * {X_SIZE})(x10)
            LOAD x22, (22 * {X_SIZE})(x10)
            LOAD x23, (23 * {X_SIZE})(x10)
            LOAD x24, (24 * {X_SIZE})(x10)
            LOAD x25, (25 * {X_SIZE})(x10)
            LOAD x26, (26 * {X_SIZE})(x10)
            LOAD x27, (27 * {X_SIZE})(x10)
            LOAD x28, (28 * {X_SIZE})(x10)
            LOAD x29, (29 * {X_SIZE})(x10)
            LOAD x30, (30 * {X_SIZE})(x10)
            LOAD x31, (31 * {X_SIZE})(x10)

            # Restore `x10`
            LOAD x10, (10 * {X_SIZE})(x10)
            ret
        ",
            X_SIZE = const X_SIZE,
            options(noreturn),
        );
    }
}
