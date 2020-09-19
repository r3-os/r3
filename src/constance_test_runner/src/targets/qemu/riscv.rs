use anyhow::Result;
use std::{future::Future, pin::Pin};

use super::super::{Arch, DebugProbe, Target, Xlen};
use super::QemuDebugProbe;

/// The RISC-V board compatible with SiFive E SDK on QEMU
pub struct QemuSiFiveE(pub Xlen);

impl Target for QemuSiFiveE {
    fn target_arch(&self) -> Arch {
        match self.0 {
            Xlen::_32 => Arch::RV32IMAC,
            Xlen::_64 => Arch::RV64IMAC,
        }
    }

    fn cargo_features(&self) -> &[&str] {
        &["output-e310x-uart", "interrupt-e310x", "board-e310x-qemu"]
    }

    fn memory_layout_script(&self) -> String {
        r#"
            MEMORY
            {
                FLASH : ORIGIN = 0x20000000, LENGTH = 16M
                RAM : ORIGIN = 0x80000000, LENGTH = 16K
            }

            REGION_ALIAS("REGION_TEXT", FLASH);
            REGION_ALIAS("REGION_RODATA", FLASH);
            REGION_ALIAS("REGION_DATA", RAM);
            REGION_ALIAS("REGION_BSS", RAM);
            REGION_ALIAS("REGION_HEAP", RAM);
            REGION_ALIAS("REGION_STACK", RAM);

            /* Skip first 4M allocated for bootloader */
            _stext = 0x20400000;

            _hart_stack_size = 1K;
        "#
        .to_owned()
    }

    fn connect(&self) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>>>>> {
        let xlen = self.0;
        Box::pin(async move {
            Ok(Box::new(QemuDebugProbe::new(
                match xlen {
                    Xlen::_32 => "qemu-system-riscv32",
                    Xlen::_64 => "qemu-system-riscv64",
                },
                &[
                    "-machine",
                    "sifive_e",
                    // UART0 → stdout
                    "-serial",
                    "file:/dev/stdout",
                    // UART1 → stderr
                    "-serial",
                    "file:/dev/stderr",
                    // Disable monitor
                    "-monitor",
                    "none",
                ],
            )) as Box<dyn DebugProbe>)
        })
    }
}

/// The RISC-V board compatible with SiFive U SDK on QEMU
pub struct QemuSiFiveU(pub Xlen);

impl Target for QemuSiFiveU {
    fn target_arch(&self) -> Arch {
        match self.0 {
            Xlen::_32 => Arch::RV32GC,
            Xlen::_64 => Arch::RV64GC,
        }
    }

    fn cargo_features(&self) -> &[&str] {
        &["output-u540-uart", "interrupt-u540-qemu", "board-u540-qemu"]
    }

    fn memory_layout_script(&self) -> String {
        r#"
            MEMORY
            {
                RAM : ORIGIN = 0x80000000, LENGTH = 16M
            }

            REGION_ALIAS("REGION_TEXT", RAM);
            REGION_ALIAS("REGION_RODATA", RAM);
            REGION_ALIAS("REGION_DATA", RAM);
            REGION_ALIAS("REGION_BSS", RAM);
            REGION_ALIAS("REGION_HEAP", RAM);
            REGION_ALIAS("REGION_STACK", RAM);

            _hart_stack_size = 1K;
            _max_hart_id = 1;
        "#
        .to_owned()
    }

    fn connect(&self) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>>>>> {
        let xlen = self.0;
        Box::pin(async move {
            Ok(Box::new(QemuDebugProbe::new(
                match xlen {
                    Xlen::_32 => "qemu-system-riscv32",
                    Xlen::_64 => "qemu-system-riscv64",
                },
                &[
                    "-machine",
                    "sifive_u",
                    "-bios",
                    "none",
                    // UART0 → stdout
                    "-serial",
                    "file:/dev/stdout",
                    // UART1 → stderr
                    "-serial",
                    "file:/dev/stderr",
                    // Disable monitor
                    "-monitor",
                    "none",
                ],
            )) as Box<dyn DebugProbe>)
        })
    }
}
