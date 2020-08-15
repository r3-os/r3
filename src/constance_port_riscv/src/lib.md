The RISC-V port for [Constance](::constance).

# Startup code

[`use_rt!`] hooks up the entry points ([`EntryPoint`]) using `#[`[`::riscv_rt::entry`]`]`. If this is not desirable for some reason, you can omit `use_rt!` and implement the code that calls the entry points in other ways.

# Interrupt Controller

Your system type should be combined with an interrupt controller driver by implementing [`PortInterrupts`] and [`InterruptController`]. Most systems are equipped with [Platform-Level Interrupt Controller (PLIC)], whose driver is provided by [`use_plic!`].

[Platform-Level Interrupt Controller (PLIC)]: https://github.com/riscv/riscv-plic-spec/blob/master/riscv-plic.adoc
[`PortInterrupts`]: constance::kernel::PortInterrupts
