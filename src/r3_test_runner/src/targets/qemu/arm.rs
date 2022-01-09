use anyhow::Result;
use std::{future::Future, pin::Pin};

use super::super::{Arch, DebugProbe, Target};
use super::QemuDebugProbe;

pub struct QemuMps2An385;

impl Target for QemuMps2An385 {
    fn target_arch(&self) -> Arch {
        Arch::CORTEX_M3
    }

    fn cargo_features(&self) -> Vec<String> {
        vec!["output-semihosting".to_owned()]
    }

    fn memory_layout_script(&self) -> String {
        "
            MEMORY
            {
              /* assuming zbt_boot_ctrl == 0 */
              FLASH : ORIGIN = 0x00000000, LENGTH = 4096k
              RAM : ORIGIN = 0x20000000, LENGTH = 4096K
            }

            _stack_start = ORIGIN(RAM) + LENGTH(RAM);
        "
        .to_owned()
    }

    fn connect(&self) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>>>>> {
        Box::pin(async {
            Ok(Box::new(QemuDebugProbe::new(
                "qemu-system-arm",
                &[
                    "-machine",
                    "mps2-an385",
                    "-semihosting",
                    "-semihosting-config",
                    "target=native",
                ],
            )) as Box<dyn DebugProbe>)
        })
    }
}

pub struct QemuMps2An505;

impl Target for QemuMps2An505 {
    fn target_arch(&self) -> Arch {
        Arch::CORTEX_M33_FPU
    }

    fn cargo_features(&self) -> Vec<String> {
        vec!["output-semihosting".to_owned()]
    }

    fn memory_layout_script(&self) -> String {
        "
            MEMORY
            {
              /* ZBT SRAM (SSRAM1) Secure alias */
              FLASH : ORIGIN = 0x10000000, LENGTH = 4096k
              /* ZBT SRAM (SSRAM2 and SSRAM3) Secure alias */
              RAM : ORIGIN = 0x38000000, LENGTH = 4096K
            }

            _stack_start = ORIGIN(RAM) + LENGTH(RAM);
        "
        .to_owned()
    }

    fn connect(&self) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>>>>> {
        Box::pin(async {
            Ok(Box::new(QemuDebugProbe::new(
                "qemu-system-arm",
                &[
                    "-machine",
                    "mps2-an505",
                    "-semihosting",
                    "-semihosting-config",
                    "target=native",
                ],
            )) as Box<dyn DebugProbe>)
        })
    }
}

/// ARM RealView Platform Baseboard Explore for Cortex-A9 on QEMU
pub struct QemuRealviewPbxA9;

impl Target for QemuRealviewPbxA9 {
    fn target_arch(&self) -> Arch {
        Arch::CORTEX_A9
    }

    fn cargo_features(&self) -> Vec<String> {
        vec!["board-realview_pbx_a9".to_owned()]
    }

    fn memory_layout_script(&self) -> String {
        // TODO: test `link_ram.x`
        "
            MEMORY
            {
              RAM_CODE : ORIGIN = 0x01000000, LENGTH = 4096K
              RAM_DATA : ORIGIN = 0x01400000, LENGTH = 4096K
            }
        "
        .to_owned()
    }

    fn connect(&self) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>>>>> {
        Box::pin(async {
            Ok(Box::new(QemuDebugProbe::new(
                "qemu-system-arm",
                &[
                    "-machine",
                    "realview-pbx-a9",
                    "-semihosting",
                    "-semihosting-config",
                    "target=native",
                ],
            )) as Box<dyn DebugProbe>)
        })
    }
}
