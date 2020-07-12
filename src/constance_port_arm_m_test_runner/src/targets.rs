use std::{
    convert::TryInto,
    error::Error,
    future::Future,
    path::Path,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio::{
    io::AsyncRead,
    task::spawn_blocking,
    time::{delay_for, Delay},
};

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
                match ProbeRsDebugProbe::new("0483:374b".try_into().unwrap(), "stm32f401re".into())
                {
                    Ok(x) => Ok(Box::new(x) as Box<dyn DebugProbe>),
                    Err(x) => Err(Box::new(x) as Box<dyn Error + Send>),
                }
            })
            .await
            .unwrap()
        })
    }
}

struct ProbeRsDebugProbe {
    session: Arc<Mutex<probe_rs::Session>>,
}

#[derive(thiserror::Error, Debug)]
enum ProbeRsDebugProbeOpenError {
    #[error("Error while opening the probe: {0}")]
    OpenProbe(#[source] probe_rs::DebugProbeError),
    #[error("Error while attaching to the probe: {0}")]
    Attach(#[source] probe_rs::Error),
}

#[derive(thiserror::Error, Debug)]
enum ProbeRsDebugProbeGetOutputError {
    #[error("Error while flashing the device: {0}")]
    Flash(#[source] probe_rs::flashing::FileDownloadError),
    #[error("Error while resetting the device: {0}")]
    Reset(#[source] probe_rs::Error),
    #[error("Error while attaching to the RTT channel: {0}")]
    AttachRtt(#[source] probe_rs_rtt::Error),
    #[error("Timeout while trying to attach to the RTT channel.")]
    AttachRttTimeout,
}

impl ProbeRsDebugProbe {
    fn new(
        probe_sel: probe_rs::DebugProbeSelector,
        target_sel: probe_rs::config::TargetSelector,
    ) -> Result<Self, ProbeRsDebugProbeOpenError> {
        let probe =
            probe_rs::Probe::open(probe_sel).map_err(ProbeRsDebugProbeOpenError::OpenProbe)?;

        let session = Arc::new(Mutex::new(
            probe
                .attach(target_sel)
                .map_err(ProbeRsDebugProbeOpenError::Attach)?,
        ));

        Ok(Self { session })
    }
}

const POLL_INTERVAL: Duration = Duration::from_millis(30);
const RTT_ATTACH_TIMEOUT: Duration = Duration::from_millis(500);

impl DebugProbe for ProbeRsDebugProbe {
    fn program_and_get_output(
        &mut self,
        exe: &Path,
    ) -> Pin<Box<dyn Future<Output = Result<DynAsyncRead<'_>, Box<dyn Error>>> + '_>> {
        let exe = exe.to_owned();
        let session = Arc::clone(&self.session);

        Box::pin(async move {
            // Flash the executable
            log::debug!("Flashing '{0}'", exe.display());

            let session2 = Arc::clone(&session);
            let exe2 = exe.clone();
            spawn_blocking(move || {
                let mut session_lock = session2.lock().unwrap();
                probe_rs::flashing::download_file(
                    &mut *session_lock,
                    &exe2,
                    probe_rs::flashing::Format::Elf,
                )
            })
            .await
            .unwrap()
            .map_err(ProbeRsDebugProbeGetOutputError::Flash)?;

            // Reset the core
            (session.lock().unwrap().core(0))
                .map_err(ProbeRsDebugProbeGetOutputError::Reset)?
                .reset()
                .map_err(ProbeRsDebugProbeGetOutputError::Reset)?;

            // Read the executable to find the RTT header
            log::debug!(
                "Reading the executable '{0}' to find the RTT header",
                exe.display()
            );
            let rtt_scan_region = match tokio::fs::read(&exe).await {
                Ok(elf_bytes) => {
                    let addr = spawn_blocking(move || find_rtt_symbol(&elf_bytes))
                        .await
                        .unwrap();
                    if let Some(x) = addr {
                        log::debug!("Found the RTT header at 0x{:x}", x);
                        probe_rs_rtt::ScanRegion::Exact(x as u32)
                    } else {
                        probe_rs_rtt::ScanRegion::Ram
                    }
                }
                Err(e) => {
                    log::warn!(
                        "Couldn't read the executable to find the RTT header: {:?}",
                        e
                    );
                    probe_rs_rtt::ScanRegion::Ram
                }
            };

            // Attach to RTT
            let start = Instant::now();
            let rtt = loop {
                match probe_rs_rtt::Rtt::attach_region(session.clone(), &rtt_scan_region) {
                    Ok(rtt) => break rtt,
                    Err(probe_rs_rtt::Error::ControlBlockNotFound) => {}
                    Err(e) => {
                        return Err(ProbeRsDebugProbeGetOutputError::AttachRtt(e).into());
                    }
                }

                if start.elapsed() > RTT_ATTACH_TIMEOUT {
                    return Err(ProbeRsDebugProbeGetOutputError::AttachRttTimeout.into());
                }

                delay_for(POLL_INTERVAL).await;
            };

            // Stream the output of all up channels
            Ok(Box::pin(ReadRtt::new(rtt)) as DynAsyncRead<'_>)
        })
    }
}

fn find_rtt_symbol(elf_bytes: &[u8]) -> Option<u64> {
    let elf = match goblin::elf::Elf::parse(elf_bytes) {
        Ok(elf) => elf,
        Err(e) => {
            log::warn!(
                "Couldn't parse the executable to find the RTT header: {:?}",
                e
            );
            return None;
        }
    };

    for sym in &elf.syms {
        if let Some(Ok(name)) = elf.strtab.get(sym.st_name) {
            if name == "_SEGGER_RTT" {
                return Some(sym.st_value);
            }
        }
    }

    None
}

struct ReadRtt {
    rtt: probe_rs_rtt::Rtt,
    poll_delay: Delay,
}

impl ReadRtt {
    fn new(rtt: probe_rs_rtt::Rtt) -> Self {
        Self {
            rtt,
            poll_delay: delay_for(POLL_INTERVAL),
        }
    }
}

impl AsyncRead for ReadRtt {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<tokio::io::Result<usize>> {
        // Read up to `buf.len()` bytes
        let mut pos = 0;
        for channel in self.rtt.up_channels().iter() {
            if pos >= buf.len() {
                break;
            }
            match channel.read(&mut buf[pos..]) {
                Ok(num_read_bytes) => {
                    if num_read_bytes > 0 {
                        log::trace!(
                            "Read {:?} from {:?}",
                            String::from_utf8_lossy(&buf[pos..][..num_read_bytes]),
                            (channel.number(), channel.name()),
                        );
                    }
                    pos += num_read_bytes;
                }
                Err(e) => {
                    return Poll::Ready(Err(tokio::io::Error::new(tokio::io::ErrorKind::Other, e)));
                }
            }
        }

        if pos == 0 {
            // Retry later
            while let Poll::Ready(()) = Pin::new(&mut self.poll_delay).poll(cx) {
                self.poll_delay = delay_for(POLL_INTERVAL);
            }
            Poll::Pending
        } else {
            Poll::Ready(Ok(pos))
        }
    }
}
