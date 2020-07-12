pub trait Target: Send + Sync {
    /// Generate the `memory.x` file to be included by `cortex-m-rt`'s linker
    /// script.
    fn memory_layout_script(&self) -> String;
}

pub static TARGETS: &[(&str, &dyn Target)] = &[("nucleo_f401re", &NucleoF401re)];

pub struct NucleoF401re;

impl Target for NucleoF401re {
    fn memory_layout_script(&self) -> String {
        "
            MEMORY
            {
              /* NOTE K = KiBi = 1024 bytes */
              FLASH : ORIGIN = 0x08000000, LENGTH = 512K
              RAM : ORIGIN = 0x20000000, LENGTH = 96K
            }

            /* This is where the call stack will be allocated. */
            /* The stack is of the full descending type. */
            /* NOTE Do NOT modify `_stack_start` unless you know what you are doing */
            _stack_start = ORIGIN(RAM) + LENGTH(RAM);
        "
        .to_owned()
    }
}
