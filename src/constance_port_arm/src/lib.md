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
