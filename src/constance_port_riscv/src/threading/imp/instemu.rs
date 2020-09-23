//! Instruction emulation
use constance_portkit::pptext::pp_asm;

use super::X_SIZE;

// TODO: add tests for `emulate-lr-sc`

/// The reserved address, used for emulating SC/LR.
pub(super) static mut RESERVATION_ADDR_VALUE: [usize; 2] = [0; 2];

/// Handle a software exception by emulating the faulting instruction.
///
/// Panics if the exception could not be resolved by instruction emulation.
///
/// We need access to callee-saved registers for instruction emulation, so
/// this method is implemented as a naked function.
///
/// # Inputs
///
///  - `a0`: pointer to FLS.X (a portion of the background context state)
///  - `a1`: `mcause`
///  - `s0-s11`: `s0-s11` from the background context state
///
#[naked]
pub(super) unsafe fn handle_exception(_fl_state: *mut usize, _mcause: usize) {
    // TODO: catch double fault
    unsafe {
        pp_asm!("
        "   crate::threading::imp::asm_inc::define_load_store!()                "
            # <a0 == fl_state, a1 == mcause>
            .cfi_startproc
            addi sp, sp, -16
            .cfi_def_cfa_offset 16
            STORE ra, (sp)
            .cfi_offset ra, -12

        "   if cfg!(feature = "emulate-lr-sc")  {                               "
                # If `mcause` ∈ [5, 7], LR/SC emulation might resolve the
                # exception.
                andi a3, a1, -3
                li a2, 5
                beq a3, a2, 9f      # TryLRSCEmulation
        "   }                                                                   "
        8:      # LRSCEmulationUnapplicable
            # <a0 == fl_state, a1 = mcause>

            # Panic.
            tail {panic_on_unhandled_exception}

        "   if cfg!(feature = "emulate-lr-sc")  {                               "
            9:      # TryLRSCEmulation
                # Read the original PC from the first-level state and
                # load the faulting instruction to `a2`.
                # The PC is only aligned by `IALIGN` bits, so split the load
                # to avoid an unaligned access exception on a target with C
                # extension.
                LOAD a2, ({X_SIZE} * 16)(a0)
                lh a3, 2(a2)
                lhu a2, (a2)
                slli a3, a3, 16
                or a2, a2, a3

                # Is it LR.W or SC.W?
                # TODO: support LR.Q/SC.Q
                li a3, 0b11110000000000000111000001111111
                li a4, 0b00010000000000000010000000101111
                and a3, a3, a2
                bne a3, a4, 8b

                # Get the target address
                srli a4, a2, 15
                call {read_x}

                # Which one is it, LR or SC?
                #
                #  t0 = a2 << (XLEN * 8 - 28)
                #  <(instruction is SC && t0 < 0) || (instruction is LR && t0 >= 0)>
                #
                slli t0, a2, 4
                bltz t0, 1f

                # Emulate the LR instruction.
                #
                #   <a2 = instruction, a4 = target, instruction is LR>
                #   target: *mut u32;
                #   a3 = SIGN_EXTEND(*target);
                #   RESERVATION_ADDR_VALUE[0] = target;
                #   RESERVATION_ADDR_VALUE[1] = a3;
                #
                lw a3, (a4)
                la t0, {RESERVATION_ADDR_VALUE}
                STORE a4, (t0)
                STORE a3, {X_SIZE}(t0)

                j 0f
            1:
                mv a5, a4

                # Get the value to be written
                srli a4, a2, 20
                call {read_x}

                # Emulate the SC instruction.
                #
                #   <a2 = instruction, a4 = value, a5 = target, instruction is SC>
                #   target: *mut u32;
                #   [t2, t1] = replace(&mut RESERVATION_ADDR_VALUE, [0, 0]);
                #   if t2 == target && t1 == SIGN_EXTEND(*target):
                #       *target = value;
                #       a3 = 0;
                #   else:
                #       a3 = 1;
                #
                la t0, {RESERVATION_ADDR_VALUE}
                LOAD t2, (t0)
                LOAD t1, {X_SIZE}(t0)
                STORE x0, (t0)
                STORE x0, {X_SIZE}(t0)
                li a3, 1
                bne t2, a5, 0f
                lw t2, (a5)
                bne t2, t1, 0f

                li a3, 0
                sw a4, (a5)

            0:
                # Get the output register
                srli a4, a2, 7

                # Write the output register.
                #
                #   <a3 = output, a4 = rd>
                #   background_state_x[rd] = output;
                #
                call {write_x}

                # Skip the current instruction.
                LOAD a3, ({X_SIZE} * 16)(a0)
                addi a3, a3, 4
                STORE a3, ({X_SIZE} * 16)(a0)

                LOAD ra, (sp)
                addi sp, sp, 16
                .cfi_def_cfa_offset 0
                ret
        "   } else {                                                            "
                # unused: {RESERVATION_ADDR_VALUE} {read_x} {write_x} {X_SIZE}
        "   }                                                                   "
            .cfi_endproc
            ",
            panic_on_unhandled_exception = sym panic_on_unhandled_exception,
            read_x = sym read_x,
            write_x = sym write_x,
            RESERVATION_ADDR_VALUE = sym RESERVATION_ADDR_VALUE,
            X_SIZE = const X_SIZE,
        );
    }
}

unsafe fn panic_on_unhandled_exception(fl_state: *mut usize, mcause: usize) -> ! {
    // Read the original PC from the first-level state
    let pc = unsafe { *fl_state.offset(16) };

    panic!("unhandled exception {} at 0x{:08x}", mcause, pc);
}

#[cfg(not(feature = "emulate-lr-sc"))]
extern "C" {
    fn read_x();
    fn write_x();
}

/// Read the `x` register specified by `a4[4:0]`. Write the result to `a4`.
/// This function trashes `t1`.
///
/// # Inputs
///
///  - `a0`: pointer to the first-level state (a portion of the background
///    context state)
///  - `a4`: The register index
///  - `s0-s11`: `s0-s11` from the background context state
///
#[naked]
#[cfg(feature = "emulate-lr-sc")]
unsafe fn read_x(_fl_state: *mut usize) {
    unsafe {
        pp_asm!("
        "   crate::threading::imp::asm_inc::define_load_store!()                "
            # <a0 == fl_state, a4 == index>
            .cfi_startproc

            # Jump to the code corresponding to the target register.
            #
            #   a4 &= 0x1f;
            #   if cfg!(target_feature = 'c'):
            #       pc = 0f + a4 * 4;
            #   else:
            #       pc = 0f + a4 * 8;
            #
            slli a4, a4, 32 - 5
        "   if cfg!(target_feature = "c") {                                     "
                srli a4, a4, 32 - 7
        "   } else {                                                            "
                srli a4, a4, 32 - 8
        "   }                                                                   "
            la t1, 0f
            add t1, t1, a4
            jr t1

        0:
            # All of the following instructions are compiled to the compressed
            # form when the C extension is enabled.
            # x0
            li a4, 0
            j 1f

            # x1/ra - first-level state
            LOAD a4, ({X_SIZE} * 0)(a0)
            j 1f

            # x2/sp - implied from the a0
            j 2f
            nop

            # x3-x4 - global
            mv a4, x3
            j 1f
            mv a4, x4
            j 1f

            # x5-x7/t0-t2 - first-level state
            LOAD a4, ({X_SIZE} * 1)(a0)
            j 1f
            LOAD a4, ({X_SIZE} * 2)(a0)
            j 1f
            LOAD a4, ({X_SIZE} * 3)(a0)
            j 1f

            # x8-x9/s0-s1 - preserved
            mv a4, x8
            j 1f
            mv a4, x9
            j 1f

            # x10-x15/a0-a5 - first-level state
            LOAD a4, ({X_SIZE} * 4)(a0)
            j 1f
            LOAD a4, ({X_SIZE} * 5)(a0)
            j 1f
            LOAD a4, ({X_SIZE} * 6)(a0)
            j 1f
            LOAD a4, ({X_SIZE} * 7)(a0)
            j 1f
            LOAD a4, ({X_SIZE} * 8)(a0)
            j 1f
            LOAD a4, ({X_SIZE} * 9)(a0)
            j 1f

            # x16-x17/a6-a7 - first-level state
            LOAD a4, ({X_SIZE} * 10)(a0)
            j 1f
            LOAD a4, ({X_SIZE} * 11)(a0)
            j 1f

            # x18-x27/s2-s11 - preserved
            mv a4, x18
            j 1f
            mv a4, x19
            j 1f
            mv a4, x20
            j 1f
            mv a4, x21
            j 1f
            mv a4, x22
            j 1f
            mv a4, x23
            j 1f
            mv a4, x24
            j 1f
            mv a4, x25
            j 1f
            mv a4, x26
            j 1f
            mv a4, x27
            j 1f

            # x28-x31/t3-t6 - first-level state
            LOAD a4, ({X_SIZE} * 12)(a0)
            j 1f
            LOAD a4, ({X_SIZE} * 13)(a0)
            j 1f
            LOAD a4, ({X_SIZE} * 14)(a0)
            j 1f
            LOAD a4, ({X_SIZE} * 15)(a0)
            j 1f

        2:
            addi a4, a0, {X_SIZE} * 17

        1:  .cfi_endproc",
            X_SIZE = const X_SIZE,
        );
    }
}

/// Update the `x` register specified by `a4[4:0]` with `a3`.
/// Trashes `t1`.
///
/// # Inputs
///
///  - `a0`: pointer to the first-level state (a portion of the background
///    context state)
///  - `a3`: The value
///  - `a4`: The register index
///  - `s0-s11`: `s0-s11` from the background context state
///
#[naked]
#[cfg(feature = "emulate-lr-sc")]
unsafe fn write_x(_fl_state: *mut usize) {
    unsafe {
        pp_asm!(
            "
            # <a0 == fl_state, a3 == value, a4 == index>
            .cfi_startproc

            # Jump to the code corresponding to the target register.
            #
            #   a4 &= 0x1f;
            #   if cfg!(target_feature = 'c'):
            #       pc = 0f + a4 * 4;
            #   else:
            #       pc = 0f + a4 * 8;
            #
            slli a4, a4, 32 - 5
        "   if cfg!(target_feature = "c") {                                     "
                srli a4, a4, 32 - 7
        "   } else {                                                            "
                srli a4, a4, 32 - 8
        "   }                                                                   "
            la t1, 0f
            add t1, t1, a4
            jr t1

        0:
            # Most of the following instructions are compiled to the compressed
            # form when the C extension is enabled.
            # x0 - no-op
            j 1f
            nop

            # x1/ra - first-level state
            sw a3, ({X_SIZE} * 0)(a0)
            j 1f

            # x2/sp - TODO
        "   if cfg!(target_feature = "c") {                                     "
                # There's no compressed form for this instruction, so this
                # instruction occupies 4 bytes
                ecall
        "   } else {                                                            "
                ecall
                nop
        "   }                                                                   "

            # x3-x4 - global
            mv x3, a3
            j 1f
            mv x4, a3
            j 1f

            # x5-x7/t0-t2 - first-level state
            STORE a3, ({X_SIZE} * 1)(a0)
            j 1f
            STORE a3, ({X_SIZE} * 2)(a0)
            j 1f
            STORE a3, ({X_SIZE} * 3)(a0)
            j 1f

            # x8-x9/s0-s1 - preserved
            mv x8, a3
            j 1f
            mv x9, a3
            j 1f

            # x10-x15/a0-a5 - first-level state
            STORE a3, ({X_SIZE} * 4)(a0)
            j 1f
            STORE a3, ({X_SIZE} * 5)(a0)
            j 1f
            STORE a3, ({X_SIZE} * 6)(a0)
            j 1f
            STORE a3, ({X_SIZE} * 7)(a0)
            j 1f
            STORE a3, ({X_SIZE} * 8)(a0)
            j 1f
            STORE a3, ({X_SIZE} * 9)(a0)
            j 1f

            # x16-x17/a6-a7 - first-level state
            STORE a3, ({X_SIZE} * 10)(a0)
            j 1f
            STORE a3, ({X_SIZE} * 11)(a0)
            j 1f

            # x18-x27/s2-s11 - preserved
            mv x18, a3
            j 1f
            mv x19, a3
            j 1f
            mv x20, a3
            j 1f
            mv x21, a3
            j 1f
            mv x22, a3
            j 1f
            mv x23, a3
            j 1f
            mv x24, a3
            j 1f
            mv x25, a3
            j 1f
            mv x26, a3
            j 1f
            mv x27, a3
            j 1f

            # x28-x31/t3-t6 - first-level state
            STORE a3, ({X_SIZE} * 12)(a0)
            j 1f
            STORE a3, ({X_SIZE} * 13)(a0)
            j 1f
            STORE a3, ({X_SIZE} * 14)(a0)
            j 1f
            STORE a3, ({X_SIZE} * 15)(a0)

        1:  .cfi_endproc",
            X_SIZE = const X_SIZE,
        );
    }
}
