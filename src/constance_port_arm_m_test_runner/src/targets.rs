use std::{convert::TryInto, error::Error, future::Future, path::Path, pin::Pin};
use tokio::{io::AsyncRead, task::spawn_blocking};

mod probe_rs;
mod qemu;

pub trait Target: Send + Sync {
    /// Get the target triple.
    ///
    ///  - `thumbv7m-none-eabi`: Armv7-M
    ///  - `thumbv7em-none-eabi`: Armv7-M + DSP
    ///  - `thumbv7em-none-eabihf`: Armv7-M + DSP + FPU
    ///
    fn target_triple(&self) -> &str;

    /// Get the additional Cargo features to enable when building
    /// `constance_port_arm_m_test_driver`.
    fn cargo_features(&self) -> &[&str];

    /// Generate the `memory.x` file to be included by `cortex-m-rt`'s linker
    /// script.
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
];

pub struct NucleoF401re;

impl Target for NucleoF401re {
    fn target_triple(&self) -> &str {
        // TODO: use `eabihf` when FPU is supported by the Arm-M port
        "thumbv7em-none-eabi"
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
        Box::pin(async { Ok(Box::new(qemu::QemuDebugProbe::new()) as Box<dyn DebugProbe>) })
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
