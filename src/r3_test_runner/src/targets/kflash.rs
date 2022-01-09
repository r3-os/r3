//! Kendryte K210 UART ISP, based on [`kflash.py`]
//! (https://github.com/sipeed/kflash.py)
use anyhow::Result;
use crc::{Crc, CRC_32_ISO_HDLC};
use std::{future::Future, marker::Unpin, path::Path, pin::Pin, sync::Mutex, time::Duration};
use tokio::{
    io::{AsyncBufRead, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufStream},
    task::spawn_blocking,
    time::sleep,
};
use tokio_serial::{SerialPort, SerialPortBuilderExt, SerialStream};

use super::{
    demux::Demux,
    serial::{choose_serial, ChooseSerialError},
    slip, Arch, DebugProbe, DynAsyncRead, Target,
};
use crate::utils::retry_on_fail;

/// Maix development boards based on Kendryte K210, download by UART ISP
pub struct Maix;

impl Target for Maix {
    fn target_arch(&self) -> Arch {
        Arch::RV64GC
    }

    fn cargo_features(&self) -> Vec<String> {
        vec![
            "boot-rt".to_owned(),
            "output-k210-uart".to_owned(),
            "interrupt-k210".to_owned(),
            "board-maix".to_owned(),
            "r3_port_riscv/maintain-pie".to_owned(),
        ]
    }

    fn memory_layout_script(&self) -> String {
        r#"
            MEMORY
            {
                RAM : ORIGIN = 0x80000000, LENGTH = 6M
            }

            REGION_ALIAS("REGION_TEXT", RAM);
            REGION_ALIAS("REGION_RODATA", RAM);
            REGION_ALIAS("REGION_DATA", RAM);
            REGION_ALIAS("REGION_BSS", RAM);
            REGION_ALIAS("REGION_HEAP", RAM);
            REGION_ALIAS("REGION_STACK", RAM);

            _hart_stack_size = 1K;
        "#
        .to_owned()
    }

    fn connect(&self) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>>>>> {
        Box::pin(async { KflashDebugProbe::new().await.map(|x| Box::new(x) as _) })
    }
}

#[derive(thiserror::Error, Debug)]
enum OpenError {
    #[error("Error while choosing the serial port to use")]
    ChooseSerial(#[source] ChooseSerialError),
    #[error("Error while opening the serial port '{0}'")]
    Serial(String, #[source] anyhow::Error),
    #[error(
        "Please provide a board name by `MAIX_BOARD` environment variable. \
        Valid values: {0:?}"
    )]
    NoBoardName(Vec<&'static str>),
    #[error("Unknown board name: '{0}'")]
    UnknownBoardName(String),
    #[error("Communication error")]
    Communication(#[source] CommunicationError),
}

#[derive(thiserror::Error, Debug)]
enum CommunicationError {
    #[error("Error while controlling the serial port")]
    Serial(#[source] tokio_serial::Error),
    #[error("Error while reading from or writing to the serial port")]
    SerialIo(
        #[source]
        #[from]
        std::io::Error,
    ),
    #[error("Protocol error")]
    FrameExtractor(#[source] slip::FrameExtractorProtocolError),
    #[error("Timeout while waiting for a response")]
    Timeout,
    #[error("Received an ISP error response {0:?}.")]
    RemoteError(IspReasonCode),
    #[error("Received a malformed response.")]
    MalformedResponse,
}

impl From<slip::FrameExtractorError> for CommunicationError {
    fn from(e: slip::FrameExtractorError) -> Self {
        match e {
            slip::FrameExtractorError::Io(e) => Self::SerialIo(e),
            slip::FrameExtractorError::Protocol(e) => Self::FrameExtractor(e),
        }
    }
}

const COMM_TIMEOUT: Duration = Duration::from_secs(3);

struct KflashDebugProbe {
    serial: BufStream<SerialStream>,
    isp_boot_cmds: &'static [BootCmd],
}

impl KflashDebugProbe {
    async fn new() -> anyhow::Result<Self> {
        // Choose the ISP sequence specific to a target board
        let board = match std::env::var("MAIX_BOARD") {
            Ok(x) => Ok(x),
            Err(std::env::VarError::NotPresent) => {
                let valid_board_names = ISP_BOOT_CMDS.iter().map(|x| x.0).collect();
                Err(OpenError::NoBoardName(valid_board_names))
            }
            Err(std::env::VarError::NotUnicode(_)) => Err(OpenError::UnknownBoardName(
                "<invalid UTF-8 string>".to_owned(),
            )),
        }?;
        let isp_boot_cmds = ISP_BOOT_CMDS
            .iter()
            .find(|x| x.0 == board)
            .ok_or_else(|| OpenError::UnknownBoardName(board.clone()))?
            .1;

        let serial = spawn_blocking(|| {
            let dev = choose_serial().map_err(OpenError::ChooseSerial)?;

            tokio_serial::new(&dev, 115200)
                .timeout(std::time::Duration::from_secs(60))
                .open_native_async()
                .map_err(|e| OpenError::Serial(dev, e.into()))
        })
        .await
        .unwrap()?;

        let serial = BufStream::new(serial);

        // Pu the device into ISP mode. Fail-fast if this was unsuccessful.
        let serial_m = Mutex::new(serial);
        retry_on_fail(|| async {
            // Holding the `LockGuard` across a suspend point is okay because
            // `Mutex::lock` is never called for this mutex. (It's practically
            // a thread-safe `RefCell`.)
            #[allow(must_not_suspend)]
            maix_enter_isp_mode(&mut serial_m.try_lock().unwrap(), isp_boot_cmds).await
        })
        .await
        .map_err(OpenError::Communication)?;
        let serial = serial_m.into_inner().unwrap();

        let probe = Self {
            serial,
            isp_boot_cmds,
        };

        Ok(probe)
    }
}

#[derive(thiserror::Error, Debug)]
enum RunError {
    #[error("{0}")]
    ProcessElf(
        #[source]
        #[from]
        ProcessElfError,
    ),
    #[error("{0}")]
    Communication(
        #[source]
        #[from]
        CommunicationError,
    ),
}

impl DebugProbe for KflashDebugProbe {
    fn program_and_get_output(
        &mut self,
        exe: &Path,
    ) -> Pin<Box<dyn Future<Output = Result<DynAsyncRead<'_>>> + '_>> {
        let exe = exe.to_owned();
        Box::pin(async move {
            // Extract loadable sections
            let LoadableCode { regions, entry } =
                read_elf(&exe).await.map_err(RunError::ProcessElf)?;

            // Put the device into ISP mode.
            let serial_m = Mutex::new(&mut self.serial);
            let isp_boot_cmds = self.isp_boot_cmds;
            retry_on_fail(|| async {
                maix_enter_isp_mode(*serial_m.try_lock().unwrap(), isp_boot_cmds).await
            })
            .await
            .map_err(RunError::Communication)?;
            drop(serial_m);

            // Program the executable image
            for (i, region) in regions.iter().enumerate() {
                log::debug!("Programming the region {} of {}", i + 1, regions.len());
                if region.1 < 0x80000000 {
                    log::debug!(
                        "Starting address (0x{:x}) is out of range, ignoreing",
                        region.1
                    );
                    continue;
                }
                flash_dataframe(&mut self.serial, &region.0, region.1 as u32).await?;
            }

            // Boot the program
            log::debug!("Booting from 0x{:08x}", entry);
            boot(&mut self.serial, entry as u32).await?;

            // Now, pass the channel to the caller
            Ok(Box::pin(Demux::new(&mut self.serial)) as _)
        })
    }
}

#[derive(Debug)]
enum BootCmd {
    Dtr(bool),
    Rts(bool),
    Delay,
}

const ISP_BOOT_CMDS: &[(&str, &[BootCmd])] = &[
    // `reset_to_isp_kd233`
    (
        "kd233",
        &[
            BootCmd::Dtr(false),
            BootCmd::Rts(false),
            BootCmd::Delay,
            BootCmd::Dtr(true),
            BootCmd::Rts(false),
            BootCmd::Delay,
            BootCmd::Rts(true),
            BootCmd::Dtr(false),
            BootCmd::Delay,
        ],
    ),
    // `reset_to_isp_dan`
    (
        "dan",
        &[
            BootCmd::Dtr(false),
            BootCmd::Rts(false),
            BootCmd::Delay,
            BootCmd::Dtr(false),
            BootCmd::Rts(true),
            BootCmd::Delay,
            BootCmd::Rts(false),
            BootCmd::Dtr(true),
            BootCmd::Delay,
        ],
    ),
    // `reset_to_isp_goD`
    (
        "god",
        &[
            BootCmd::Dtr(true),
            BootCmd::Rts(true),
            BootCmd::Delay,
            BootCmd::Rts(false),
            BootCmd::Dtr(true),
            BootCmd::Delay,
            BootCmd::Rts(false),
            BootCmd::Dtr(true),
            BootCmd::Delay,
        ],
    ),
    // `reset_to_boot_maixgo`
    (
        "maixgo",
        &[
            BootCmd::Dtr(false),
            BootCmd::Rts(false),
            BootCmd::Delay,
            BootCmd::Rts(false),
            BootCmd::Dtr(true),
            BootCmd::Delay,
            BootCmd::Rts(false),
            BootCmd::Dtr(false),
            BootCmd::Delay,
        ],
    ),
];

async fn maix_enter_isp_mode(
    serial: &mut BufStream<SerialStream>,
    cmds: &[BootCmd],
) -> Result<(), CommunicationError> {
    let t = Duration::from_millis(100);

    let serial_inner = serial.get_mut();
    log::debug!("Trying to put the chip into ISP mode");
    for cmd in cmds {
        log::trace!("Performing the command {:?}", cmd);
        match cmd {
            BootCmd::Dtr(b) => {
                serial_inner
                    .write_data_terminal_ready(*b)
                    .map_err(CommunicationError::Serial)?;
            }
            BootCmd::Rts(b) => {
                serial_inner
                    .write_request_to_send(*b)
                    .map_err(CommunicationError::Serial)?;
            }
            BootCmd::Delay => {
                sleep(t).await;
            }
        }
    }

    // Clear any stale data in the receive buffer
    read_to_end_and_discard_for_some_time(serial).await?;

    // Send a greeting command
    log::trace!("Sending a greeting command");
    slip::write_frame(
        serial,
        &[
            0xc2, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
    )
    .await
    .map_err(CommunicationError::SerialIo)?;
    serial.flush().await.map_err(CommunicationError::SerialIo)?;

    // Wait for a response
    log::trace!("Waiting for a response");
    match tokio::time::timeout(COMM_TIMEOUT, slip::read_frame(serial)).await {
        Ok(Ok(frame)) => {
            log::trace!(
                "Received a packet: {:?} The chip probably successfully entered ISP mode",
                frame
            );
        }
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(CommunicationError::Timeout),
    }

    Ok(())
}

async fn flash_dataframe(
    serial: &mut (impl AsyncBufRead + AsyncWrite + Unpin),
    data: &[u8],
    address: u32,
) -> Result<(), CommunicationError> {
    const CHUNK_LEN: usize = 1024;
    let mut buffer = [0u8; 8 + CHUNK_LEN];
    buffer[0] = 0xc3;

    for (i, chunk) in data.chunks(CHUNK_LEN).enumerate() {
        let chunk_addr = address + (i * CHUNK_LEN) as u32;

        log::debug!(
            "Programming the range {:?}/{:?} at 0x{:x} ({}%)",
            (i * CHUNK_LEN)..(i * CHUNK_LEN + chunk.len()),
            data.len(),
            chunk_addr,
            i * CHUNK_LEN * 100 / data.len(),
        );

        let mut error = None;

        for _ in 0..16 {
            buffer[0..][..4].copy_from_slice(&chunk_addr.to_le_bytes());
            buffer[4..][..4].copy_from_slice(&(chunk.len() as u32).to_le_bytes());
            buffer[8..][..chunk.len()].copy_from_slice(chunk);

            // Send a frame
            log::trace!("Sending a write command");
            write_request(serial, 0xc3, &buffer[..8 + chunk.len()])
                .await
                .map_err(CommunicationError::SerialIo)?;
            serial.flush().await.map_err(CommunicationError::SerialIo)?;

            // Wait for a response
            let response = match tokio::time::timeout(COMM_TIMEOUT, slip::read_frame(serial)).await
            {
                Ok(Ok(frame)) => frame,
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => return Err(CommunicationError::Timeout),
            };

            let reason: Option<IspReasonCode> = response.get(1).cloned().map(Into::into);

            match reason {
                Some(IspReasonCode::Ok) => {
                    error = None;
                    break;
                }
                Some(x) => {
                    error = Some(CommunicationError::RemoteError(x));
                }
                None => {
                    error = Some(CommunicationError::MalformedResponse);
                }
            }

            log::trace!("Got {:?}. Retrying...", reason);
        }

        if let Some(error) = error {
            return Err(error);
        }
    }

    Ok(())
}

async fn boot(
    serial: &mut (impl AsyncWrite + Unpin),
    address: u32,
) -> Result<(), CommunicationError> {
    let mut buffer = [0u8; 8];
    buffer[..4].copy_from_slice(&address.to_le_bytes());

    // Send a frame
    log::trace!("Sending a boot command");
    write_request(serial, 0xc5, &buffer)
        .await
        .map_err(CommunicationError::SerialIo)?;
    serial.flush().await.map_err(CommunicationError::SerialIo)?;

    Ok(())
}

async fn write_request(
    serial: &mut (impl AsyncWrite + Unpin),
    cmd: u8,
    req_payload: &[u8],
) -> std::io::Result<()> {
    let mut frame_payload = vec![0u8; req_payload.len() + 8];
    frame_payload[0] = cmd;
    frame_payload[8..].copy_from_slice(req_payload);

    let crc = Crc::<u32>::new(&CRC_32_ISO_HDLC).checksum(req_payload);
    frame_payload[4..][..4].copy_from_slice(&crc.to_le_bytes());

    slip::write_frame(serial, &frame_payload).await
}

#[derive(Debug, Copy, Clone)]
enum IspReasonCode {
    Default,
    Ok,
    BadDataLen,
    BadDataChecksum,
    InvalidCommand,
    BadInitialization,
    BadExec,
    Unknown(u8),
}

impl From<u8> for IspReasonCode {
    fn from(x: u8) -> Self {
        match x {
            0x00 => Self::Default,
            0xe0 => Self::Ok,
            0xe1 => Self::BadDataLen,
            0xe2 => Self::BadDataChecksum,
            0xe3 => Self::InvalidCommand,
            0xe4 => Self::BadInitialization,
            0xe5 => Self::BadExec,
            x => Self::Unknown(x),
        }
    }
}

async fn read_to_end_and_discard_for_some_time(
    reader: &mut (impl AsyncRead + Unpin),
) -> std::io::Result<()> {
    log::trace!("Starting discarding stale data in the receive buffer");
    match tokio::time::timeout(Duration::from_millis(100), read_to_end_and_discard(reader)).await {
        // FIXME: This match arm is really unreachable because `Infallible` is
        //        uninhabited. Waiting for `exhaustive_patterns` feature
        //        <https://github.com/rust-lang/rust/issues/51085>
        Ok(Ok(_)) => unreachable!(),
        Ok(Err(e)) => Err(e),
        Err(_) => Ok(()),
    }
}

async fn read_to_end_and_discard(
    reader: &mut (impl AsyncRead + Unpin),
) -> std::io::Result<std::convert::Infallible> {
    let mut buf = [0u8; 256];
    loop {
        let num_bytes = reader.read(&mut buf).await?;
        log::trace!("Discarding {} byte(s)", num_bytes);
    }
}

#[derive(thiserror::Error, Debug)]
enum ProcessElfError {
    #[error("Couldn't read the ELF file: {0}")]
    Read(#[source] std::io::Error),
    #[error("Couldn't parse the ELF file: {0}")]
    Parse(#[source] goblin::error::Error),
}

struct LoadableCode {
    /// The regions to be loaded onto the target.
    regions: Vec<(Vec<u8>, u64)>,
    /// The entry point.
    entry: u64,
}

/// Read the specified ELF file and return regions to be loaded onto the target.
async fn read_elf(exe: &Path) -> Result<LoadableCode, ProcessElfError> {
    let elf_bytes = tokio::fs::read(&exe).await.map_err(ProcessElfError::Read)?;
    let elf = goblin::elf::Elf::parse(&elf_bytes).map_err(ProcessElfError::Parse)?;

    let regions = elf
        .program_headers
        .iter()
        .filter_map(|ph| {
            if ph.p_type == goblin::elf32::program_header::PT_LOAD && ph.p_filesz > 0 {
                Some((
                    elf_bytes[ph.p_offset as usize..][..ph.p_filesz as usize].to_vec(),
                    ph.p_paddr,
                ))
            } else {
                None
            }
        })
        .collect();

    Ok(LoadableCode {
        regions,
        entry: elf.entry,
    })
}
