use std::{convert::TryInto, error::Error, future::Future, path::Path, pin::Pin};
use tokio::{io::AsyncRead, task::spawn_blocking};

mod jlink;
mod probe_rs;
mod qemu;

pub trait Target: Send + Sync {
    /// Get the target triple.
    ///
    ///  - `armv7a-none-eabi`: Armv7-A
    ///  - `thumbv7m-none-eabi`: Armv7-M
    ///  - `thumbv7em-none-eabi`: Armv7-M + DSP
    ///  - `thumbv7em-none-eabihf`: Armv7-M + DSP + FPU
    ///  - `riscv32imac-unknown-none-elf`: RISC-V RV32I + Multiplication and
    ///    Division + Atomics + Compressed Instructions
    ///
    fn target_triple(&self) -> &str;

    /// Get the additional Cargo features to enable when building
    /// `constance_port_arm_m_test_driver`.
    fn cargo_features(&self) -> &[&str];

    /// Get a flag indicating whether `libcore` should be built with the
    /// smallest configuration.
    fn small_rt(&self) -> bool {
        false
    }

    /// Generate the `memory.x` file to be included by the linker script of
    /// `cortex-m-rt` or `constance_port_arm`, or to be used as the top-level
    /// linker script by `constance_port_riscv_test_driver`.
    fn memory_layout_script(&self) -> String;

    /// Connect to the target.
    fn connect(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>, Box<dyn Error + Send>>>>>;
}

pub trait DebugProbe: Send + Sync {
    /// Program the specified ELF image and run it from the beginning to
    /// capture its output.
    fn program_and_get_output(
        &mut self,
        exe: &Path,
    ) -> Pin<Box<dyn Future<Output = Result<DynAsyncRead<'_>, Box<dyn Error>>> + '_>>;
}

type DynAsyncRead<'a> = Pin<Box<dyn AsyncRead + 'a>>;

pub static TARGETS: &[(&str, &dyn Target)] = &[
    ("nucleo_f401re", &NucleoF401re),
    ("qemu_mps2_an385", &QemuMps2An385),
    // QEMU doesn't provide any predefined machine with Armv6-M, so just use
    // the Armv7-M machine
    (
        "qemu_mps2_an385_v6m",
        &OverrideTargetTriple("thumbv6m-none-eabi", QemuMps2An385),
    ),
    (
        "qemu_mps2_an505",
        &OverrideTargetTriple("thumbv8m.main-none-eabihf", QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v8mml",
        &OverrideTargetTriple("thumbv8m.main-none-eabi", QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v8mbl",
        &OverrideTargetTriple("thumbv8m.base-none-eabi", QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v7em_hf",
        &OverrideTargetTriple("thumbv7em-none-eabihf", QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v7m_hf",
        &OverrideTargetTriple("thumbv7m-none-eabihf", QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v7em",
        &OverrideTargetTriple("thumbv7em-none-eabi", QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v7m",
        &OverrideTargetTriple("thumbv7m-none-eabi", QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v6m",
        &OverrideTargetTriple("thumbv6m-none-eabi", QemuMps2An505),
    ),
    ("qemu_realview_pbx_a9", &QemuRealviewPbxA9),
    ("qemu_sifive_e", &QemuSiFiveE),
    ("red_v", &RedV),
    ("red_v_itim", &RedVItim),
];

pub struct NucleoF401re;

impl Target for NucleoF401re {
    fn target_triple(&self) -> &str {
        "thumbv7em-none-eabihf"
    }

    fn cargo_features(&self) -> &[&str] {
        &["output-rtt"]
    }

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

    fn connect(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>, Box<dyn Error + Send>>>>> {
        Box::pin(async {
            spawn_blocking(|| {
                match probe_rs::ProbeRsDebugProbe::new(
                    "0483:374b".try_into().unwrap(),
                    "stm32f401re".into(),
                ) {
                    Ok(x) => Ok(Box::new(x) as Box<dyn DebugProbe>),
                    Err(x) => Err(Box::new(x) as Box<dyn Error + Send>),
                }
            })
            .await
            .unwrap()
        })
    }
}

pub struct QemuMps2An385;

impl Target for QemuMps2An385 {
    fn target_triple(&self) -> &str {
        "thumbv7m-none-eabi"
    }

    fn cargo_features(&self) -> &[&str] {
        &["output-semihosting"]
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

    fn connect(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>, Box<dyn Error + Send>>>>> {
        Box::pin(async {
            Ok(Box::new(qemu::QemuDebugProbe::new(
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
    fn target_triple(&self) -> &str {
        "thumbv8m-none-eabihf"
    }

    fn cargo_features(&self) -> &[&str] {
        &["output-semihosting"]
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

    fn connect(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>, Box<dyn Error + Send>>>>> {
        Box::pin(async {
            Ok(Box::new(qemu::QemuDebugProbe::new(
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
    fn target_triple(&self) -> &str {
        "armv7a-none-eabi"
    }

    fn cargo_features(&self) -> &[&str] {
        &["output-semihosting"]
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

    fn connect(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>, Box<dyn Error + Send>>>>> {
        Box::pin(async {
            Ok(Box::new(qemu::QemuDebugProbe::new(
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

/// The RISC-V board compatible with SiFive E SDK on QEMU
pub struct QemuSiFiveE;

impl Target for QemuSiFiveE {
    fn target_triple(&self) -> &str {
        "riscv32imac-unknown-none-elf"
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

    fn connect(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>, Box<dyn Error + Send>>>>> {
        Box::pin(async {
            Ok(Box::new(qemu::QemuDebugProbe::new(
                "qemu-system-riscv32",
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

/// SparkFun RED-V RedBoard or Things Plus
pub struct RedV;

impl Target for RedV {
    fn target_triple(&self) -> &str {
        "riscv32imac-unknown-none-elf"
    }

    fn cargo_features(&self) -> &[&str] {
        &[
            "output-rtt",
            "interrupt-e310x",
            "board-e310x-red-v",
            "constance_port_riscv/emulate-lr-sc",
        ]
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

            /* Skip first 64K allocated for bootloader */
            _stext = 0x20010000;

            _hart_stack_size = 1K;
        "#
        .to_owned()
    }

    fn connect(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>, Box<dyn Error + Send>>>>> {
        Box::pin(std::future::ready(Ok(
            Box::new(jlink::Fe310JLinkDebugProbe::new()) as _,
        )))
    }
}

/// SparkFun RED-V RedBoard or Things Plus, code execution on ITIM
/// (Instruction Tightly Integrated Memory). Some code still executes on flash
/// because of the size restriction of ITIM.
pub struct RedVItim;

impl Target for RedVItim {
    fn target_triple(&self) -> &str {
        RedV.target_triple()
    }

    fn cargo_features(&self) -> &[&str] {
        RedV.cargo_features()
    }

    fn small_rt(&self) -> bool {
        true
    }

    fn memory_layout_script(&self) -> String {
        r#"
            MEMORY
            {
                FLASH_TEXT : ORIGIN = 0x20100000, LENGTH = 1M
                FLASH_RODATA : ORIGIN = 0x20200000, LENGTH = 1M
                ITIM : ORIGIN = 0x08000000, LENGTH = 16K
                DTIM : ORIGIN = 0x80000000, LENGTH = 16K
            }

            REGION_ALIAS("REGION_TEXT", ITIM);
            REGION_ALIAS("REGION_RODATA", FLASH_RODATA);
            REGION_ALIAS("REGION_DATA", DTIM);
            REGION_ALIAS("REGION_BSS", DTIM);
            REGION_ALIAS("REGION_HEAP", DTIM);
            REGION_ALIAS("REGION_STACK", DTIM);

            _hart_stack_size = 1K;

            /* it's impossible to put the whole application in ITIM, so place
             * only the code relevant to benchmarking in ITIM and the rest in
             * flash */
            SECTIONS
            {
                .text.cold :
                {
                    /* formatting and loogging*/
                    *(.text.*fmt..Debug*);
                    *(.text.*fmt..Display*);
                    *(.text.*core3fmt*);
                    *(.text.*rtt_target*);

                    /* startup */
                    *(.text.*port_boot*);
                    *(.text.*setup_interrupts*);
                    *(.text.*timer_tick*);
                    *(.text.*e310x_hal*);
                    *(.text.*e310x4init*);
                    *(.text.*Kernel$GT$4boot*);

                    /* instruction emulation */
                    *(.text.*instemu6read_x*);
                    *(.text.*instemu6write_x*);

                    /* timer */
                    *(.text.*combined_handler*);

                    /* reporting and error handling */
                    *(.text.*report_success*);
                    *(.text.*panic*);

                    /* miscellaneous */
                    *(.text.*bcmp*);
                    *(.text.*memset*);
                    *(.text.*memcpy*);
                    *(.text.*__multi3*);
                    *(.text.*__udivdi3*);
                    *(.text.*__udivmoddi*);
                } > FLASH_TEXT
            }
        "#
        .to_owned()
    }

    fn connect(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>, Box<dyn Error + Send>>>>> {
        Box::pin(std::future::ready(Ok(
            Box::new(jlink::Fe310JLinkDebugProbe::new()) as _,
        )))
    }
}

pub struct OverrideTargetTriple<T>(&'static str, T);

impl<T: Target> Target for OverrideTargetTriple<T> {
    fn target_triple(&self) -> &str {
        self.0
    }

    fn cargo_features(&self) -> &[&str] {
        self.1.cargo_features()
    }

    fn memory_layout_script(&self) -> String {
        self.1.memory_layout_script()
    }

    fn connect(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>, Box<dyn Error + Send>>>>> {
        self.1.connect()
    }
}
