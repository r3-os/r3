use std::{
    convert::TryInto,
    error::Error,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio::io::AsyncRead;

pub trait Target: Send + Sync {
    /// Generate the `memory.x` file to be included by `cortex-m-rt`'s linker
    /// script.
    fn memory_layout_script(&self) -> String;

    /// Connect to the target.
    fn connect(&self)
        -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>, Box<dyn Error>>>>>;
}

pub trait DebugProbe {
    /// Program the specified ELF image and run it from the beginning and
    /// capturing its output.
    fn program_and_get_output(
        &mut self,
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
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>, Box<dyn Error>>>>> {
        Box::pin(async {
            Ok(Box::new(ProbeRsDebugProbe::new(
                "0483:374b".try_into().unwrap(),
                "stm32f401re".into(),
            )?) as Box<dyn DebugProbe>)
        })
    }
}

struct ProbeRsDebugProbe {
    session: Arc<Mutex<probe_rs::Session>>,
}

impl ProbeRsDebugProbe {
    fn new(
        probe_sel: probe_rs::DebugProbeSelector,
        target_sel: probe_rs::config::TargetSelector,
    ) -> Result<Self, Box<dyn Error>> {
        let probe = probe_rs::Probe::open(probe_sel)?;

        let session = Arc::new(Mutex::new(probe.attach(target_sel)?));

        Ok(Self { session })
    }
}

impl DebugProbe for ProbeRsDebugProbe {
    fn program_and_get_output(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<DynAsyncRead<'_>, Box<dyn Error>>> + '_>> {
        Box::pin(async { todo!() })
    }
}
