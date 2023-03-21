use anyhow::Result;
use async_mutex::{Mutex as AsyncMutex, MutexGuardArc as AsyncMutexGuard};
use futures_core::ready;
use std::{
    future::Future,
    io::Write,
    mem::replace,
    path::Path,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufRead, AsyncRead, ReadBuf},
    task::{spawn_blocking, JoinHandle},
    time::{sleep, Sleep},
};

use super::{Arch, DebugProbe, DynAsyncRead, LinkerScripts, Target};

pub struct NucleoF401re;

impl Target for NucleoF401re {
    fn target_arch(&self) -> Arch {
        Arch::CORTEX_M4F
    }

    fn cargo_features(&self) -> Vec<String> {
        vec!["output-rtt".to_owned()]
    }

    fn linker_scripts(&self) -> LinkerScripts {
        LinkerScripts::arm_m_rt(
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
            .to_owned(),
        )
    }
    fn connect(&self) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>>>>> {
        Box::pin(async {
            spawn_blocking(|| {
                ProbeRsDebugProbe::new("0483:374b".try_into().unwrap(), "stm32f401re".into())
                    .map(|x| Box::new(x) as _)
            })
            .await
            .unwrap()
        })
    }
}

struct ProbeRsDebugProbe {
    session: Arc<AsyncMutex<probe_rs::Session>>,
}

#[derive(thiserror::Error, Debug)]
enum OpenError {
    #[error("Error while opening the probe")]
    OpenProbe(#[source] probe_rs::DebugProbeError),
    #[error("Error while attaching to the probe")]
    Attach(#[source] probe_rs::Error),
}

#[derive(thiserror::Error, Debug)]
enum RunError {
    #[error("Error while flashing the device")]
    Flash(#[source] probe_rs::flashing::FileDownloadError),
    #[error("Error while resetting the device")]
    Reset(#[source] probe_rs::Error),
}

impl ProbeRsDebugProbe {
    fn new(
        probe_sel: probe_rs::DebugProbeSelector,
        target_sel: probe_rs::config::TargetSelector,
    ) -> anyhow::Result<Self> {
        let probe = probe_rs::Probe::open(probe_sel).map_err(OpenError::OpenProbe)?;

        let session = Arc::new(AsyncMutex::new(
            probe
                .attach(target_sel, probe_rs::Permissions::new())
                .map_err(OpenError::Attach)?,
        ));

        Ok(Self { session })
    }
}

impl DebugProbe for ProbeRsDebugProbe {
    fn program_and_get_output(
        &mut self,
        exe: &Path,
    ) -> Pin<Box<dyn Future<Output = Result<DynAsyncRead<'_>>> + '_>> {
        let exe = exe.to_owned();

        Box::pin(async move {
            let mut session = self.session.lock_arc().await;

            // Flash the executable
            log::debug!("Flashing '{0}'", exe.display());

            let exe2 = exe.clone();
            let mut session = spawn_blocking(move || {
                probe_rs::flashing::download_file(
                    &mut session,
                    &exe2,
                    probe_rs::flashing::Format::Elf,
                )?;
                Ok(session)
            })
            .await
            .unwrap()
            .map_err(RunError::Flash)?;

            // Reset the core
            session
                .core(0)
                .map_err(RunError::Reset)?
                .reset()
                .map_err(RunError::Reset)?;

            // Attach to RTT
            Ok(attach_rtt(session, &exe, Default::default()).await?)
        })
    }
}

const POLL_INTERVAL: Duration = Duration::from_millis(30);
const RTT_ATTACH_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(thiserror::Error, Debug)]
pub enum AttachRttError {
    #[error("Error while attaching to the RTT channel")]
    AttachRtt(#[source] probe_rs::rtt::Error),
    #[error("Error while halting or resuming the core to access the RTT channel")]
    HaltCore(#[source] probe_rs::Error),
    #[error("Timeout while trying to attach to the RTT channel.")]
    Timeout,
}

#[derive(Default)]
pub struct RttOptions {
    /// When set to `true`, the core is halted whenever accessing RTT.
    pub halt_on_access: bool,
}

pub async fn attach_rtt(
    mut session: AsyncMutexGuard<probe_rs::Session>,
    exe: &Path,
    options: RttOptions,
) -> Result<DynAsyncRead<'static>, AttachRttError> {
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
                log::debug!("Found the RTT header at {x:#x}");
                probe_rs::rtt::ScanRegion::Exact(x as u32)
            } else {
                probe_rs::rtt::ScanRegion::Ram
            }
        }
        Err(e) => {
            log::warn!("Couldn't read the executable to find the RTT header: {e:?}");
            probe_rs::rtt::ScanRegion::Ram
        }
    };

    // Attach to RTT
    let start = Instant::now();
    let rtt = loop {
        let halt_on_access = options.halt_on_access;
        let rtt_scan_region = rtt_scan_region.clone();

        let (result, session2) = spawn_blocking(move || {
            let memory_map = session.target().memory_map.clone();
            let result = {
                let mut core = session.core(0).map_err(AttachRttError::HaltCore)?;
                let halt_guard;
                let core = if halt_on_access {
                    halt_guard = CoreHaltGuard::new(&mut core).map_err(AttachRttError::HaltCore)?;
                    &mut *halt_guard.core
                } else {
                    &mut core
                };

                match probe_rs::rtt::Rtt::attach_region(core, &memory_map, &rtt_scan_region) {
                    Ok(rtt) => Some(rtt),
                    Err(probe_rs::rtt::Error::ControlBlockNotFound) => None,
                    Err(e) => return Err(AttachRttError::AttachRtt(e)),
                }
            };
            Ok((result, session))
        })
        .await
        .unwrap()?;
        session = session2;

        if let Some(rtt) = result {
            break rtt;
        }

        if start.elapsed() > RTT_ATTACH_TIMEOUT {
            return Err(AttachRttError::Timeout);
        }

        sleep(POLL_INTERVAL).await;
    };

    // Stream the output of all up channels
    Ok(Box::pin(ReadRtt::new(session, rtt, options)) as DynAsyncRead<'_>)
}

fn find_rtt_symbol(elf_bytes: &[u8]) -> Option<u64> {
    let elf = match goblin::elf::Elf::parse(elf_bytes) {
        Ok(elf) => elf,
        Err(e) => {
            log::warn!("Couldn't parse the executable to find the RTT header: {e:?}");
            return None;
        }
    };

    for sym in &elf.syms {
        if let Some(name) = elf.strtab.get_at(sym.st_name) {
            if name == "_SEGGER_RTT" {
                return Some(sym.st_value);
            }
        }
    }

    None
}

/// Halts the first core while this RAII guard is held.
struct CoreHaltGuard<'a, 'probe> {
    core: &'a mut probe_rs::Core<'probe>,
}

impl<'a, 'probe> CoreHaltGuard<'a, 'probe> {
    fn new(core: &'a mut probe_rs::Core<'probe>) -> Result<Self, probe_rs::Error> {
        core.halt(std::time::Duration::from_millis(100))?;
        Ok(Self { core })
    }
}

impl Drop for CoreHaltGuard<'_, '_> {
    fn drop(&mut self) {
        if let Err(e) = self.core.run() {
            log::warn!("Failed to restart the core (ignored): {e:?}");
        }
    }
}

struct ReadRtt {
    options: RttOptions,
    st: ReadRttSt,
}

enum ReadRttSt {
    /// `ReadRtt` has some data in a buffer and is ready to return it through
    /// `<ReadRtt as AsyncRead>`.
    Idle {
        buf: ReadRttBuf,
        session: AsyncMutexGuard<probe_rs::Session>,
        rtt: Box<probe_rs::rtt::Rtt>,
        pos: usize,
        len: usize,
    },

    /// `ReadRtt` is currently fetching new data from RTT channels.
    Read {
        join_handle: JoinHandle<
            tokio::io::Result<(
                ReadRttBuf,
                AsyncMutexGuard<probe_rs::Session>,
                usize,
                Box<probe_rs::rtt::Rtt>,
            )>,
        >,
    },

    /// `ReadRtt` is waiting for some time before trying reading again.
    PollDelay {
        buf: ReadRttBuf,
        session: AsyncMutexGuard<probe_rs::Session>,
        rtt: Box<probe_rs::rtt::Rtt>,
        delay: Pin<Box<Sleep>>,
    },

    Invalid,
}

type ReadRttBuf = Box<[u8; 1024]>;

impl ReadRtt {
    fn new(
        session: AsyncMutexGuard<probe_rs::Session>,
        rtt: probe_rs::rtt::Rtt,
        options: RttOptions,
    ) -> Self {
        Self {
            options,
            st: ReadRttSt::Idle {
                buf: Box::new([0u8; _]),
                session,
                rtt: Box::new(rtt),
                pos: 0,
                len: 0,
            },
        }
    }
}

impl AsyncRead for ReadRtt {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<tokio::io::Result<()>> {
        // Naïve implementation of `poll_read` that uses `<Self as AsyncBufRead>`
        let my_buf = ready!(Pin::as_mut(&mut self).poll_fill_buf(cx))?;
        let num_bytes_read = my_buf.len().min(buf.remaining());
        buf.put_slice(&my_buf[..num_bytes_read]);
        Pin::as_mut(&mut self).consume(num_bytes_read);
        Poll::Ready(Ok(()))
    }
}

impl AsyncBufRead for ReadRtt {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<tokio::io::Result<&[u8]>> {
        let this = Pin::into_inner(self);

        loop {
            match &mut this.st {
                &mut ReadRttSt::Idle { pos, len, .. } => {
                    if pos == len {
                        // Buffer is empty; start reading RTT channels
                        let ReadRttSt::Idle { mut buf, mut rtt, mut session, .. } =
                            replace(&mut this.st, ReadRttSt::Invalid)
                        else { unreachable!() };

                        let halt_on_access = this.options.halt_on_access;

                        // Reading RTT is a blocking operation, so do it in a
                        // separate thread
                        let join_handle = spawn_blocking(move || {
                            let num_read_bytes = Self::read_inner(
                                &mut session,
                                &mut rtt,
                                &mut *buf,
                                halt_on_access,
                            )?;

                            // Send the buffer back to the `ReadRtt`
                            Ok((buf, session, num_read_bytes, rtt))
                        });

                        this.st = ReadRttSt::Read { join_handle };
                    } else {
                        // We have some data to return.
                        //
                        // Borrow `this.st` again, this time using the full
                        // lifetime of `self`.
                        let ReadRttSt::Idle { buf, .. } = &this.st else { unreachable!() };
                        return Poll::Ready(Ok(&buf[..len][pos..]));
                    }
                }

                ReadRttSt::Read { join_handle } => {
                    let (buf, session, num_read_bytes, rtt) =
                        match ready!(Pin::new(join_handle).poll(cx)).unwrap() {
                            Ok(x) => x,
                            Err(e) => return Poll::Ready(Err(e)),
                        };

                    this.st = if num_read_bytes == 0 {
                        // If no bytes were read, wait for a while and try again
                        ReadRttSt::PollDelay {
                            buf,
                            session,
                            rtt,
                            delay: Box::pin(sleep(POLL_INTERVAL)),
                        }
                    } else {
                        ReadRttSt::Idle {
                            buf,
                            session,
                            rtt,
                            pos: 0,
                            len: num_read_bytes,
                        }
                    };
                }

                ReadRttSt::PollDelay { delay, .. } => {
                    ready!(delay.as_mut().poll(cx));

                    let ReadRttSt::PollDelay { buf, rtt, session, .. } =
                        replace(&mut this.st, ReadRttSt::Invalid)
                    else { unreachable!() };

                    this.st = ReadRttSt::Idle {
                        buf,
                        session,
                        rtt,
                        pos: 0,
                        len: 0,
                    };
                }

                ReadRttSt::Invalid => unreachable!(),
            }
        }
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        let ReadRttSt::Idle { pos, len, .. } = &mut self.st else { unreachable!() };
        *pos += amt;
        assert!(*pos <= *len);
    }
}

impl ReadRtt {
    fn read_inner(
        session: &mut probe_rs::Session,
        rtt: &mut probe_rs::rtt::Rtt,
        buf: &mut [u8],
        halt_on_access: bool,
    ) -> tokio::io::Result<usize> {
        let mut core = session
            .core(0)
            .map_err(|e| tokio::io::Error::new(tokio::io::ErrorKind::Other, e))?;
        let halt_guard;
        let core = if halt_on_access {
            halt_guard = CoreHaltGuard::new(&mut core)
                .map_err(|e| tokio::io::Error::new(tokio::io::ErrorKind::Other, e))?;
            &mut *halt_guard.core
        } else {
            &mut core
        };

        let mut num_read_bytes = 0;

        for (i, channel) in rtt.up_channels().iter().enumerate() {
            let num_ch_read_bytes = channel
                .read(core, buf)
                .map_err(|e| tokio::io::Error::new(tokio::io::ErrorKind::Other, e))?;

            if num_ch_read_bytes != 0 {
                log::trace!(
                    "Read {:?} from {:?}",
                    String::from_utf8_lossy(&buf[..num_ch_read_bytes]),
                    (channel.number(), channel.name()),
                );

                if i == 0 {
                    // Terminal channel - send it to `ReadRtt`.
                    // Don't bother checking other channels because we don't
                    // want `buf` to be overwritten with a log channel's payload.
                    num_read_bytes = num_ch_read_bytes;
                    break;
                } else {
                    // Log channel - send it to stdout
                    std::io::stdout()
                        .write_all(&buf[..num_ch_read_bytes])
                        .unwrap();
                }
            }
        }

        Ok(num_read_bytes)
    }
}
