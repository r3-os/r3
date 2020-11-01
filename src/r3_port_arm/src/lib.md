The Arm-A port for [the R3 kernel](::r3).

# Startup Code

[`use_startup!`] generates an entry point (with a symbol name `start`), which is expected to be called by a bootloader. The startup code configures MMU to assign appropriate memory attributes based on the memory map supplied by [`StartupOptions::MEMORY_MAP`] and to map an exception vector table at `0x0000_0000` or `0xffff_0000`.

## Linker Scripts

This crate provides linker scripts that define some standard sections, suitable for use with a bootloader that can handle ELF sections. Put files as follows under your crate's directory:

`.cargo/config.toml`:

```toml
[target.armv7a-none-eabi]
rustflags = ["-C", "link-arg=-Tlink_ram.x"]
```

`memory.x`:

```text
MEMORY
{
  RAM : ORIGIN = 0x20000000, LENGTH = 10240K
}
```

`build.rs`:

```rust,ignore
fn main() {
    // Use the linker script `memory.x` at the crate root
    println!(
        "cargo:rustc-link-search={}",
        std::env::current_dir().unwrap().display()
    );
}
```

The following linker scripts are provided:

 - `link_ram.x` places all sections in `RAM`.
 - `link_ram_harvard.x` places `.text` in `RAM_CODE` and all remaining sections in `RAM_DATA`. Combined with an approriate MMU configuration, this can be used to implement the W⊕X ([write xor execute]) memory policy for enhanced security. It might also lead to a performance improvement on a processor having separate buses for instruction and data access.

[write xor execute]: https://en.wikipedia.org/wiki/W%5EX

# Kernel Timing

As far as kernel timing is concerned, there is no universal solution for a Cortex-A system. This crate provides a port timer driver for [Arm PrimeCell SP804 Dual Timer], which can be instantiated by [`use_sp804!`].

[Arm PrimeCell SP804 Dual Timer]: https://developer.arm.com/documentation/ddi0271/d/

# Interrupt Controller

Your system type should be combined with an interrupt controller driver by implementing [`PortInterrupts`] and [`InterruptController`]. Most systems are equipped with Arm Generic Interrupt Controller (GIC), whose driver is provided by [`use_gic!`].

The maximum possible range of valid interrupt numbers is `0..1020` (the upper bound varies across implementations). The range is statically partitioned as follows:

 - `0..16` is used for SGIs (Software-Generated Interrupts), which are used for inter-processor communication. SGIs don't support enabling, disabling, or changing their trigger modes.
 - `16..32` is used for PPIs (Private Peripheral Interrupts), which are peripheral interrupts specific to a single processor.
 - `32..` is used for SPIs (Shared Peripheral Interrupts), which are peripheral interrupts that the Distributor can route to a specified set of processors. The current implementation of the GIC driver routes all interrupts to CPU 0, assuming that's where the application runs.

The valid priority range is `0..255`. All priorities are [*managed*] - unmanaged interrupts aren't supported yet.

The GIC driver exposes additional operations on interrupt lines through [`Gic`] implemented on your system type.

[`PortInterrupts`]: r3::kernel::PortInterrupts
[*managed*]: r3::kernel::PortInterrupts::MANAGED_INTERRUPT_PRIORITY_RANGE

# Implementation

## Context state

The state of an interrupted thread is stored to the interrupted thread's stack in the following form:

```rust,ignore
#[repr(C)]
struct ContextState {
    // Second-level state
    //
    // Includes everything that is not included in the first-level state. These
    // are moved between memory and registers only when switching tasks.
    // TODO: Floating-point registers
    r4: u32,
    r5: u32,
    r6: u32,
    r7: u32,
    r8: u32,
    r9: u32,
    r10: u32,
    r11: u32,

    // First-level state
    //
    // This was designed after Arm-M's exception frame.
    //
    // The GPR potion is comprised of caller-saved registers. In an exception
    // handler, saving/restoring this set of registers at entry and exit allows
    // it to call Rust functions.
    //
    // `{pc, cpsr}` is the sequence of registers that the RFE (return from
    // exception) instruction expects to be in memory in this exact order.
    r0: u32,
    r1: u32,
    r2: u32,
    r3: u32,
    r12: u32,
    lr: u32,
    pc: u32,
    cpsr: u32,
}
```

`sp` is stored in [`TaskCb::port_task_state`].

[`TaskCb::port_task_state`]: r3::kernel::TaskCb::port_task_state

When a task is activated, a new context state is created inside the task's stack. By default, only essential registers are preloaded with known values. The **`preload-registers`** Cargo feature enables preloading for all GPRs, which might help in debugging at the cost of performance and code size.

For the idle task, saving and restoring the context store is essentially replaced with no-op or loads of hard-coded values. In particular, `pc` is always “restored” with the entry point of the idle task.

## Processor Modes

 - **System**: Task context. The idle task (the implicit task that runs when `*`[`running_task_ptr`]`().is_none()`) uses this mode with `sp_usr == 0` (no other tasks or non-task contexts use `sp == 0`, so this is straightforward to detect).
 - **Supervisor**: Non-task context
 - **IRQ**: The processor enters this mode when it takes an exception. This state lasts only briefly because the IRQ handler switches to Supervisor as soon as possible to allow reentry. `sp_irq` is only used as a scratch register.

[`running_task_ptr`]: r3::kernel::State::running_task_ptr
