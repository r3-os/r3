use anyhow::Result;
use std::{future::Future, path::Path, pin::Pin};
use tokio::io::AsyncRead;

mod demux;
mod jlink;
mod kflash;
mod openocd;
mod probe_rs;
mod qemu;
mod serial;
mod slip;

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

    /// Extra target feature flags.
    fn target_features(&self) -> &str {
        ""
    }

    /// Get the additional Cargo features to enable when building
    /// `constance_port_arm_m_test_driver`.
    fn cargo_features(&self) -> &[&str];

    /// Generate the `memory.x` file to be included by the linker script of
    /// `cortex-m-rt` or `constance_port_arm`, or to be used as the top-level
    /// linker script by `constance_port_riscv_test_driver`.
    fn memory_layout_script(&self) -> String;

    /// Connect to the target.
    fn connect(&self) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>>>>>;
}

pub trait DebugProbe: Send {
    /// Program the specified ELF image and run it from the beginning to
    /// capture its output.
    fn program_and_get_output(
        &mut self,
        exe: &Path,
    ) -> Pin<Box<dyn Future<Output = Result<DynAsyncRead<'_>>> + '_>>;
}

type DynAsyncRead<'a> = Pin<Box<dyn AsyncRead + 'a>>;

pub static TARGETS: &[(&str, &dyn Target)] = &[
    ("nucleo_f401re", &probe_rs::NucleoF401re),
    ("qemu_mps2_an385", &qemu::arm::QemuMps2An385),
    // QEMU doesn't provide any predefined machine with Armv6-M, so just use
    // the Armv7-M machine
    (
        "qemu_mps2_an385_v6m",
        &OverrideTargetTriple("thumbv6m-none-eabi", qemu::arm::QemuMps2An385),
    ),
    (
        "qemu_mps2_an505",
        &OverrideTargetTriple("thumbv8m.main-none-eabihf", qemu::arm::QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v8mml",
        &OverrideTargetTriple("thumbv8m.main-none-eabi", qemu::arm::QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v8mbl",
        &OverrideTargetTriple("thumbv8m.base-none-eabi", qemu::arm::QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v7em_hf",
        &OverrideTargetTriple("thumbv7em-none-eabihf", qemu::arm::QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v7m_hf",
        &OverrideTargetTriple("thumbv7m-none-eabihf", qemu::arm::QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v7em",
        &OverrideTargetTriple("thumbv7em-none-eabi", qemu::arm::QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v7m",
        &OverrideTargetTriple("thumbv7m-none-eabi", qemu::arm::QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v6m",
        &OverrideTargetTriple("thumbv6m-none-eabi", qemu::arm::QemuMps2An505),
    ),
    ("qemu_realview_pbx_a9", &qemu::arm::QemuRealviewPbxA9),
    ("gr_peach", &openocd::GrPeach),
    (
        "qemu_sifive_e_rv32",
        &qemu::riscv::QemuSiFiveE(qemu::riscv::Xlen::_32),
    ),
    (
        "qemu_sifive_e_rv64",
        &qemu::riscv::QemuSiFiveE(qemu::riscv::Xlen::_64),
    ),
    ("qemu_sifive_u_rv32", &qemu::riscv::QemuSiFiveURv32),
    ("qemu_sifive_u_rv64", &qemu::riscv::QemuSiFiveURv64),
    ("red_v", &jlink::RedV),
    ("maix", &kflash::Maix),
];

struct OverrideTargetTriple<T>(&'static str, T);

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

    fn connect(&self) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>>>>> {
        self.1.connect()
    }
}
