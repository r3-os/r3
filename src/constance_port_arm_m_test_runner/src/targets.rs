use std::{convert::TryInto, error::Error, future::Future, path::Path, pin::Pin};
use tokio::{io::AsyncRead, task::spawn_blocking};

mod probe_rs;

pub trait Target: Send + Sync {
    /// Generate the `memory.x` file to be included by `cortex-m-rt`'s linker
    /// script.
    fn memory_layout_script(&self) -> String;

    /// Connect to the target.
    fn connect(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>, Box<dyn Error + Send>>>>>;
}

pub trait DebugProbe: Send + Sync {
    /// Program the specified ELF image and run it from the beginning and
    /// capturing its output.
    fn program_and_get_output(
        &mut self,
        exe: &Path,
    ) -> Pin<Box<dyn Future<Output = Result<DynAsyncRead<'_>, Box<dyn Error>>> + '_>>;
}

type DynAsyncRead<'a> = Pin<Box<dyn AsyncRead + 'a>>;

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
