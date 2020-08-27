//! Instruction emulation
use constance_portkit::pptext::pp_asm;

// TODO: add tests for `emulate-lr-sc`

/// The reserved address, used for emulating SC/LR.
pub(super) static mut RESERVATION_ADDR: usize = 0;

/// Handle a software exception by emulating the faulting instruction.
///
/// Panics if the exception could not be resolved by instruction emulation.
///
/// We need access to callee-saved registers for instruction emulation, so
/// this method is implemented as a naked function.
///
/// # Inputs
///
///  - `a0`: pointer to the first-level state (a portion of the background
///    context state)
///  - `a1`: `mcause`
///  - `s0-s11`: `s0-s11` from the background context state
///
#[naked]
pub(super) unsafe fn handle_exception(_fl_state: *mut usize, _mcause: usize) {
    // TODO: catch double fault
    unsafe {
        pp_asm!("
            # <a0 == fl_state, a1 == mcause>
            .cfi_startproc
            addi sp, sp, -16
            .cfi_def_cfa_offset 16
            sw ra, (sp)
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
                lw a2, (4 * 16)(a0)
                lh a3, 2(a2)
                lhu a2, (a2)
                slli a3, a3, 16
                or a2, a2, a3

                # Is it LR.W or SC.W?
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
                #   a3 = *target;
                #   RESERVATION_ADDR = target;
                #
                lw a3, (a4)
                sw a4, ({RESERVATION_ADDR}), t0

                j 0f
            1:
                mv a5, a4

                # Get the value to be written
                srli a4, a2, 20
                call {read_x}

                # Emulate the SC instruction.
                #
                #   <a2 = instruction, a4 = value, a5 = target, instruction is SC>
                #   t2 = replace(&mut RESERVATION_ADDR, 0);
                #   if t2 == target:
                #       *target = value;
                #       a3 = 0;
                #   else:
                #       a3 = 1;
                #
                lw t2, ({RESERVATION_ADDR})
                sw x0, ({RESERVATION_ADDR}), t0
                li a3, 1
                bne t2, a5, 0f

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
                lw a3, (4 * 16)(a0)
                addi a3, a3, 4
                sw a3, (4 * 16)(a0)

                lw ra, (sp)
                addi sp, sp, 16
                .cfi_def_cfa_offset 0
                ret
        "   } else {                                                            "
                # unused: {RESERVATION_ADDR} {read_x} {write_x}
        "   }                                                                   "
            .cfi_endproc
            ",
            panic_on_unhandled_exception = sym panic_on_unhandled_exception,
            read_x = sym read_x,
            write_x = sym write_x,
            RESERVATION_ADDR = sym RESERVATION_ADDR,
        );
    }
}

unsafe fn panic_on_unhandled_exception(fl_state: *mut usize, mcause: usize) -> ! {
    // Read the original PC from the first-level state
    let pc = unsafe { *fl_state.offset(16) };

    panic!("unhandled exception {} at 0x{:08x}", mcause, pc);
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
unsafe fn read_x(_fl_state: *mut usize) {
    unsafe {
        asm!(
            "
            # <a0 == fl_state, a4 == index>
            .cfi_startproc

            # Jump to the code corresponding to the target register.
            # TODO: Support non-C target
            #
            #   a4 &= 0x1f;
            #   pc = 0f + a4 * 4;
            #
            slli a4, a4, 32 - 5
            srli a4, a4, 32 - 7
            la t1, 0f
            add t1, t1, a4
            jr t1

        0:
            # x0
            c.li a4, 0
            c.j 1f

            # x1/ra - first-level state
            c.lw a4, (4 * 0)(a0)
            c.j 1f

            # x2/sp - implied from the a0
            c.j 2f
            c.nop

            # x3-x4 - global
            c.mv a4, x3
            c.j 1f
            c.mv a4, x4
            c.j 1f

            # x5-x7/t0-t2 - first-level state
            c.lw a4, (4 * 1)(a0)
            c.j 1f
            c.lw a4, (4 * 2)(a0)
            c.j 1f
            c.lw a4, (4 * 3)(a0)
            c.j 1f

            # x8-x9/s0-s1 - preserved
            c.mv a4, x8
            c.j 1f
            c.mv a4, x9
            c.j 1f

            # x10-x15/a0-a5 - first-level state
            c.lw a4, (4 * 4)(a0)
            c.j 1f
            c.lw a4, (4 * 5)(a0)
            c.j 1f
            c.lw a4, (4 * 6)(a0)
            c.j 1f
            c.lw a4, (4 * 7)(a0)
            c.j 1f
            c.lw a4, (4 * 8)(a0)
            c.j 1f
            c.lw a4, (4 * 9)(a0)
            c.j 1f

            # x16-x17/a6-a7 - first-level state
            c.lw a4, (4 * 10)(a0)
            c.j 1f
            c.lw a4, (4 * 11)(a0)
            c.j 1f

            # x18-x27/s2-s11 - preserved
            c.mv a4, x18
            c.j 1f
            c.mv a4, x19
            c.j 1f
            c.mv a4, x20
            c.j 1f
            c.mv a4, x21
            c.j 1f
            c.mv a4, x22
            c.j 1f
            c.mv a4, x23
            c.j 1f
            c.mv a4, x24
            c.j 1f
            c.mv a4, x25
            c.j 1f
            c.mv a4, x26
            c.j 1f
            c.mv a4, x27
            c.j 1f

            # x28-x31/t3-t6 - first-level state
            c.lw a4, (4 * 12)(a0)
            c.j 1f
            c.lw a4, (4 * 13)(a0)
            c.j 1f
            c.lw a4, (4 * 14)(a0)
            c.j 1f
            c.lw a4, (4 * 15)(a0)
            c.j 1f

        2:
            addi a4, a0, 4 * 17

        1:  .cfi_endproc"
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
unsafe fn write_x(_fl_state: *mut usize) {
    unsafe {
        asm!(
            "
            # <a0 == fl_state, a3 == value, a4 == index>
            .cfi_startproc

            # Jump to the code corresponding to the target register.
            # TODO: Support non-C target
            #
            #   a4 &= 0x1f;
            #   pc = 0f + a4 * 4;
            #
            slli a4, a4, 32 - 5
            srli a4, a4, 32 - 7
            la t1, 0f
            add t1, t1, a4
            jr t1

        0:
            # x0 - no-op
            c.j 1f
            c.nop

            # x1/ra - first-level state
            c.sw a3, (4 * 0)(a0)
            c.j 1f

            # x2/sp - TODO
            ecall

            # x3-x4 - global
            c.mv x3, a3
            c.j 1f
            c.mv x4, a3
            c.j 1f

            # x5-x7/t0-t2 - first-level state
            c.sw a3, (4 * 1)(a0)
            c.j 1f
            c.sw a3, (4 * 2)(a0)
            c.j 1f
            c.sw a3, (4 * 3)(a0)
            c.j 1f

            # x8-x9/s0-s1 - preserved
            c.mv x8, a3
            c.j 1f
            c.mv x9, a3
            c.j 1f

            # x10-x15/a0-a5 - first-level state
            c.sw a3, (4 * 4)(a0)
            c.j 1f
            c.sw a3, (4 * 5)(a0)
            c.j 1f
            c.sw a3, (4 * 6)(a0)
            c.j 1f
            c.sw a3, (4 * 7)(a0)
            c.j 1f
            c.sw a3, (4 * 8)(a0)
            c.j 1f
            c.sw a3, (4 * 9)(a0)
            c.j 1f

            # x16-x17/a6-a7 - first-level state
            c.sw a3, (4 * 10)(a0)
            c.j 1f
            c.sw a3, (4 * 11)(a0)
            c.j 1f

            # x18-x27/s2-s11 - preserved
            c.mv x18, a3
            c.j 1f
            c.mv x19, a3
            c.j 1f
            c.mv x20, a3
            c.j 1f
            c.mv x21, a3
            c.j 1f
            c.mv x22, a3
            c.j 1f
            c.mv x23, a3
            c.j 1f
            c.mv x24, a3
            c.j 1f
            c.mv x25, a3
            c.j 1f
            c.mv x26, a3
            c.j 1f
            c.mv x27, a3
            c.j 1f

            # x28-x31/t3-t6 - first-level state
            c.sw a3, (4 * 12)(a0)
            c.j 1f
            c.sw a3, (4 * 13)(a0)
            c.j 1f
            c.sw a3, (4 * 14)(a0)
            c.j 1f
            c.sw a3, (4 * 15)(a0)

        1:  .cfi_endproc"
        );
    }
}
