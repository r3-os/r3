The RISC-V port for [Constance](::constance).

# Startup code

[`use_rt!`] hooks up the entry points ([`EntryPoint`]) using `#[`[`::riscv_rt::entry`]`]`. If this is not desirable for some reason, you can omit `use_rt!` and implement the code that calls the entry points in other ways.

# Interrupt Controller

Your system type should be combined with an interrupt controller driver by implementing [`PortInterrupts`] and [`InterruptController`]. Most systems are equipped with [Platform-Level Interrupt Controller (PLIC)], whose driver is provided by [`use_plic!`].

[Platform-Level Interrupt Controller (PLIC)]: https://github.com/riscv/riscv-plic-spec/blob/master/riscv-plic.adoc
[`PortInterrupts`]: constance::kernel::PortInterrupts

PLIC does not support pending or clearing interrupt lines.

# Implementation

The CPU Lock state is mapped to `mstatus.MIE` (global interrupt-enable). Unmanaged interrupts aren't supported.

## Context State

The state of an interrupted thread is stored to the interrupted thread's stack in the following form:

```rust,ignore
#[repr(C)]
struct ContextState {
    // Second-level state
    //
    // Includes everything that is not included in the first-level state. These
    // are moved between memory and registers only when switching tasks.
    // TODO: Floating-point registers
    x8: usize,  // s0/fp
    x9: usize,  // s1
    #[cfg(not(e))]
    x18: usize, // s2
    #[cfg(not(e))]
    x19: usize, // s3
    #[cfg(not(e))]
    x20: usize, // s4
    #[cfg(not(e))]
    x21: usize, // s5
    #[cfg(not(e))]
    x22: usize, // s6
    #[cfg(not(e))]
    x23: usize, // s7
    #[cfg(not(e))]
    x24: usize, // s8
    #[cfg(not(e))]
    x25: usize, // s9
    #[cfg(not(e))]
    x26: usize, // s10
    #[cfg(not(e))]
    x27: usize, // s11

    // First-level state
    //
    // The GPR potion is comprised of callee-saved registers. In an exception
    // handler, saving/restoring this set of registers at entry and exit allows
    // it to call Rust functions.
    //
    // The registers are ordered in the encoding order (rather than grouping
    // them by their purposes, as done by Linux and FreeBSD) to improve the
    // compression ratio very slightly when transmitting the code over a
    // network.
    x1: usize,  // ra
    x5: usize,  // t0
    x6: usize,  // t1
    x7: usize,  // t2
    x10: usize, // a0
    x11: usize, // a1
    x12: usize, // a2
    x13: usize, // a3
    x14: usize, // a4
    x15: usize, // a5
    #[cfg(not(e))]
    x16: usize, // a6
    #[cfg(not(e))]
    x17: usize, // a7
    #[cfg(not(e))]
    x28: usize, // t3
    #[cfg(not(e))]
    x29: usize, // t4
    #[cfg(not(e))]
    x30: usize, // t5
    #[cfg(not(e))]
    x31: usize, // t6
    pc: usize, // original program counter
}
```

`x2` (`sp`) is stored in [`TaskCb::port_task_state`]. The stored stack pointer is only aligned to word boundaries.

[`TaskCb::port_task_state`]: constance::kernel::TaskCb::port_task_state

The idle task (the implicit task that runs when `*`[`running_task_ptr`]`().is_none()`) always execute with `sp == 0`. For the idle task, saving and restoring the context store is essentially replaced with no-op or loads of hard-coded values. In particular, `pc` is always “restored” with the entry point of the idle task.

## Processor Modes

All code executes in Machine mode. The value of `mstatus.MPP` is always `M` (`0b11`).
