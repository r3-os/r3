use super::{Arch, DebugProbe, Target};

pub struct RaspberryPiPico;

impl Target for RaspberryPiPico {
    fn target_arch(&self) -> Arch {
        Arch::CORTEX_M0
    }

    fn cargo_features(&self) -> &[&str] {
        &["output-rtt"] // TODO
    }

    fn memory_layout_script(&self) -> String {
        "
            MEMORY
            {
              /* NOTE K = KiBi = 1024 bytes */
              BOOT_LOADER : ORIGIN = 0x10000000, LENGTH = 0x100
              FLASH : ORIGIN = 0x10000100, LENGTH = 2048K
              RAM : ORIGIN = 0x20000000, LENGTH = 264K
            }

            SECTIONS {
              .boot_loader ORIGIN(BOOT_LOADER) :
              {
                KEEP(*(.boot_loader*));
              } > BOOT_LOADER
            } INSERT BEFORE .text;

            /* This is where the call stack will be allocated. */
            /* The stack is of the full descending type. */
            /* NOTE Do NOT modify `_stack_start` unless you know what you are doing */
            _stack_start = ORIGIN(RAM) + LENGTH(RAM);
        "
        .to_owned()
    }

    fn connect(
        &self,
    ) -> std::pin::Pin<Box<dyn futures_core::Future<Output = anyhow::Result<Box<dyn DebugProbe>>>>>
    {
        todo!()
    }
}
