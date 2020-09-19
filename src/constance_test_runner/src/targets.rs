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
    /// Get the target architecture.
    fn target_arch(&self) -> Arch;

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
        &OverrideTargetArch(Arch::CORTEX_M0, qemu::arm::QemuMps2An385),
    ),
    ("qemu_mps2_an505", &qemu::arm::QemuMps2An505),
    (
        "qemu_mps2_an505_v8mml",
        &OverrideTargetArch(Arch::CORTEX_M33, qemu::arm::QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v8mbl",
        &OverrideTargetArch(Arch::CORTEX_M23, qemu::arm::QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v7em_hf",
        &OverrideTargetArch(Arch::CORTEX_M4F, qemu::arm::QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v7m_hf",
        &OverrideTargetArch(
            Arch::ArmM {
                version: ArmMVersion::Armv7M,
                fpu: true,
                dsp: false,
            },
            qemu::arm::QemuMps2An505,
        ),
    ),
    (
        "qemu_mps2_an505_v7em",
        &OverrideTargetArch(Arch::CORTEX_M4, qemu::arm::QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v7m",
        &OverrideTargetArch(Arch::CORTEX_M3, qemu::arm::QemuMps2An505),
    ),
    (
        "qemu_mps2_an505_v6m",
        &OverrideTargetArch(Arch::CORTEX_M0, qemu::arm::QemuMps2An505),
    ),
    ("qemu_realview_pbx_a9", &qemu::arm::QemuRealviewPbxA9),
    ("gr_peach", &openocd::GrPeach),
    ("qemu_sifive_e_rv32", &qemu::riscv::QemuSiFiveE(Xlen::_32)),
    ("qemu_sifive_e_rv64", &qemu::riscv::QemuSiFiveE(Xlen::_64)),
    ("qemu_sifive_u_rv32", &qemu::riscv::QemuSiFiveU(Xlen::_32)),
    ("qemu_sifive_u_rv64", &qemu::riscv::QemuSiFiveU(Xlen::_64)),
    ("red_v", &jlink::RedV),
    ("maix", &kflash::Maix),
];

struct OverrideTargetArch<T>(Arch, T);

impl<T: Target> Target for OverrideTargetArch<T> {
    fn target_arch(&self) -> Arch {
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

#[derive(Debug, Copy, Clone)]
pub enum Arch {
    /// Armv7-A
    Armv7A,
    /// Arm M-Profile
    ArmM {
        /// Specifies the architecture version to use.
        version: ArmMVersion,
        /// The Floating-point extension.
        fpu: bool,
        /// The DSP extension.
        dsp: bool,
    },
    Riscv {
        /// XLEN
        xlen: Xlen,
        /// The "M" extension (multiplication and division)
        m: bool,
        /// The "A" extension (atomics)
        a: bool,
        /// The "C" extension (compressed instructions)
        c: bool,
        /// The "F" extension (single-precision floating point numbers)
        f: bool,
        /// The "D" extension (double-precision floating point numbers)
        d: bool,
    },
}

#[derive(Debug, Copy, Clone)]
pub enum ArmMVersion {
    Armv6M,
    Armv7M,
    Armv8MBaseline,
    Armv8MMainline,
}

#[derive(Debug, Copy, Clone)]
pub enum Xlen {
    _32,
    _64,
}

/// A set of build options passed to `rustc` to build an application for some
/// target specified by [`Arch`].
#[derive(Default)]
pub struct BuildOpt {
    pub target_triple: &'static str,
    pub target_features: String,
}

impl Arch {
    const CORTEX_A9: Self = Self::Armv7A;

    const CORTEX_M0: Self = Self::ArmM {
        version: ArmMVersion::Armv6M,
        fpu: false,
        dsp: false,
    };
    const CORTEX_M3: Self = Self::ArmM {
        version: ArmMVersion::Armv7M,
        fpu: false,
        dsp: false,
    };
    const CORTEX_M4: Self = Self::ArmM {
        version: ArmMVersion::Armv7M,
        fpu: false,
        dsp: true,
    };
    const CORTEX_M4F: Self = Self::ArmM {
        version: ArmMVersion::Armv7M,
        fpu: true,
        dsp: true,
    };
    const CORTEX_M23: Self = Self::ArmM {
        version: ArmMVersion::Armv8MBaseline,
        fpu: false,
        dsp: false,
    };
    const CORTEX_M33: Self = Self::ArmM {
        version: ArmMVersion::Armv8MMainline,
        fpu: false,
        dsp: false,
    };
    const CORTEX_M33_FPU: Self = Self::ArmM {
        version: ArmMVersion::Armv8MMainline,
        fpu: true,
        dsp: false,
    };

    const RV32IMAC: Self = Self::Riscv {
        xlen: Xlen::_32,
        m: true,
        a: true,
        c: true,
        f: false,
        d: false,
    };

    const RV64IMAC: Self = Self::Riscv {
        xlen: Xlen::_64,
        m: true,
        a: true,
        c: true,
        f: false,
        d: false,
    };

    const RV32GC: Self = Self::Riscv {
        xlen: Xlen::_32,
        m: true,
        a: true,
        c: true,
        f: true,
        d: true,
    };

    const RV64GC: Self = Self::Riscv {
        xlen: Xlen::_64,
        m: true,
        a: true,
        c: true,
        f: true,
        d: true,
    };

    pub fn build_opt(&self) -> Option<BuildOpt> {
        match self {
            // Arm A-Profile
            // -------------------------------------------------------------
            Self::Armv7A => Some(BuildOpt::from_target_triple("armv7a-none-eabi")),

            // Arm M-Profile
            // -------------------------------------------------------------
            Self::ArmM {
                version: ArmMVersion::Armv6M,
                fpu: false,
                dsp: false,
            } => Some(BuildOpt::from_target_triple("thumbv6m-none-eabi")),

            Self::ArmM {
                version: ArmMVersion::Armv6M,
                fpu: _,
                dsp: _,
            } => None,

            Self::ArmM {
                version: ArmMVersion::Armv7M,
                fpu: false,
                dsp: false,
            } => Some(BuildOpt::from_target_triple("thumbv7m-none-eabi")),

            Self::ArmM {
                version: ArmMVersion::Armv7M,
                fpu: false,
                dsp: true,
            } => Some(BuildOpt::from_target_triple("thumbv7em-none-eabi")),

            Self::ArmM {
                version: ArmMVersion::Armv7M,
                fpu: true,
                dsp: true,
            } => Some(BuildOpt::from_target_triple("thumbv7em-none-eabihf")),

            Self::ArmM {
                version: ArmMVersion::Armv7M,
                fpu: true,
                dsp: false,
            } => None,

            Self::ArmM {
                version: ArmMVersion::Armv8MBaseline,
                fpu: false,
                dsp: false,
            } => Some(BuildOpt::from_target_triple("thumbv8m.base-none-eabi")),

            Self::ArmM {
                version: ArmMVersion::Armv8MMainline,
                fpu: false,
                dsp: false,
            } => Some(BuildOpt::from_target_triple("thumbv8m.main-none-eabi")),

            Self::ArmM {
                version: ArmMVersion::Armv8MMainline,
                fpu: true,
                dsp: false,
            } => Some(BuildOpt::from_target_triple("thumbv8m.main-none-eabihf")),

            Self::ArmM {
                version: ArmMVersion::Armv8MBaseline | ArmMVersion::Armv8MMainline,
                fpu: _,
                dsp: _,
            } => None,

            // RISC-V
            // -------------------------------------------------------------
            Self::Riscv {
                xlen: Xlen::_32,
                m: false,
                a: false,
                c: false,
                f: false,
                d: false,
            } => Some(BuildOpt::from_target_triple("riscv32i-unknown-none-elf")),

            Self::Riscv {
                xlen: Xlen::_32,
                m: true,
                a: false,
                c: true,
                f: false,
                d: false,
            } => Some(BuildOpt::from_target_triple("riscv32imc-unknown-none-elf")),

            Self::Riscv {
                xlen: Xlen::_32,
                m: true,
                a: true,
                c: true,
                f: false,
                d: false,
            } => Some(BuildOpt::from_target_triple("riscv32imac-unknown-none-elf")),

            Self::Riscv {
                xlen: Xlen::_64,
                m: true,
                a: true,
                c: true,
                f: false,
                d: false,
            } => Some(BuildOpt::from_target_triple("riscv64imac-unknown-none-elf")),

            Self::Riscv {
                xlen: Xlen::_64,
                m: true,
                a: true,
                c: true,
                f: true,
                d: true,
            } => Some(BuildOpt::from_target_triple("riscv64gc-unknown-none-elf")),

            Self::Riscv {
                xlen,
                m,
                a,
                c,
                f,
                d,
            } => Some(
                BuildOpt::from_target_triple(match xlen {
                    Xlen::_32 => "riscv32imac-unknown-none-elf",
                    Xlen::_64 => "riscv64imac-unknown-none-elf",
                })
                .with_target_features(&[
                    if *m { None } else { Some("-m") },
                    if *a { None } else { Some("-a") },
                    if *c { None } else { Some("-c") },
                    if *f { Some("+f") } else { None },
                    if *d { Some("+d") } else { None },
                ]),
            ),
        }
    }
}

impl BuildOpt {
    fn from_target_triple(target_triple: &'static str) -> Self {
        Self {
            target_triple,
            ..Default::default()
        }
    }

    fn with_target_features(self, seq: &[Option<&'static str>]) -> Self {
        Self {
            target_features: crate::utils::CommaSeparatedNoSpace(seq.iter().filter_map(|x| *x))
                .to_string(),
            ..self
        }
    }
}
