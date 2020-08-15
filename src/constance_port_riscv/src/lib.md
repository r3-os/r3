The RISC-V port for [Constance](::constance).

# Startup code

[`use_rt!`] hooks up the entry points ([`EntryPoint`]) using `#[`[`::riscv_rt::entry`]`]`. If this is not desirable for some reason, you can omit `use_rt!` and implement the code that calls the entry points in other ways.