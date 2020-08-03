The Arm-A port for [Constance](::constance).

TODO

# Startup Code

[`use_startup!`] generates an entry point (with a symbol name `start`), which is expected to be called by a bootloader. The startup code configures MMU to assign appropriate memory attributes based on the memory map supplied by [`StartupOptions::MEMORY_MAP`] and to map an exception vector table at `0x0000_0000` or `0xffff_0000`.

This crate also provides a linker script `link_ram.x` that defines some standard sections, suitable for use with a bootloader that can handle ELF sections. Put files as follows under your crate's directory:

`.cargo/config.toml`:

```toml
[target.armv7a-none-eabi]
rustflags = ["-C", "link-arg=-Tlink_ram.x"]
```

`link_ram.x`:

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
    // The GPR potion is comprised of callee-saved registers. In an exception
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

[`TaskCb::port_task_state`]: constance::kernel::TaskCb::port_task_state

For the idle task, saving and restoring the context store is essentially replaced with no-op or loads of hard-coded values. In particular, `pc` is always “restored” with the entry point of the idle task.

## Processor Modes

 - **System**: Task context. The idle task (the implicit task that runs when `*`[`running_task_ptr`]`().is_none()`) uses this mode with `sp_usr == 0` (no other tasks or non-task contexts use `sp == 0`, so this is straightforward to detect).
 - **Supervisor**: Non-task context
 - **IRQ**: The processor enters this mode when it takes an exception. This state lasts only briefly because the IRQ handler switches to Supervisor as soon as possible to allow reentry. `sp_irq` is only used as a scratch register.

[`running_task_ptr`]: constance::kernel::State::running_task_ptr
