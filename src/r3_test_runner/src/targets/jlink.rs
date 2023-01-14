use anyhow::Result;
use async_mutex::Mutex as AsyncMutex;
use std::{
    fmt::Write,
    future::Future,
    io,
    path::Path,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tempdir::TempDir;
use tokio::{
    io::{AsyncRead, ReadBuf},
    process::Child,
    task::spawn_blocking,
};

use super::{Arch, DebugProbe, DynAsyncRead, LinkerScripts, Target};
use crate::subprocess;

/// SparkFun RED-V RedBoard or Things Plus
pub struct RedV;

impl Target for RedV {
    fn target_arch(&self) -> Arch {
        Arch::RV32IMAC
    }

    fn cargo_features(&self) -> Vec<String> {
        vec![
            "boot-rt".to_owned(),
            "output-rtt".to_owned(),
            "interrupt-e310x".to_owned(),
            "timer-clint".to_owned(),
            "board-e310x-red-v".to_owned(),
            "r3_port_riscv/emulate-lr-sc".to_owned(),
        ]
    }

    fn linker_scripts(&self) -> LinkerScripts {
        LinkerScripts::riscv_rt(
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
            .to_owned(),
        )
    }
    fn connect(&self) -> Pin<Box<dyn Future<Output = Result<Box<dyn DebugProbe>>>>> {
        Box::pin(std::future::ready(Ok(Box::new(Fe310JLinkDebugProbe) as _)))
    }
}

#[derive(thiserror::Error, Debug)]
enum RunError {
    #[error("Error while analyzing the ELF file")]
    ProcessElf(#[source] ProcessElfError),
    #[error("Error while creating a temporary directory")]
    CreateTempDir(#[source] std::io::Error),
    #[error("Error while creating a temporary file")]
    CreateTempFile(#[source] std::io::Error),
    #[error("Error while flashing the device")]
    Flash(#[source] subprocess::SubprocessError),
    #[error("Error while opening the probe")]
    OpenProbe(#[source] probe_rs::DebugProbeError),
    #[error("Error while attaching to the probe")]
    Attach(#[source] probe_rs::Error),
}

struct Fe310JLinkDebugProbe;

impl DebugProbe for Fe310JLinkDebugProbe {
    fn program_and_get_output(
        &mut self,
        exe: &Path,
    ) -> Pin<Box<dyn Future<Output = Result<DynAsyncRead<'_>>> + '_>> {
        let exe = exe.to_owned();
        Box::pin(async move {
            // Extract loadable sections
            let LoadableCode { regions, entry } =
                read_elf(&exe).await.map_err(RunError::ProcessElf)?;

            // Extract loadable regions to separate binary files
            let tempdir = TempDir::new("r3_test_runner").map_err(RunError::CreateTempDir)?;
            let section_files: Vec<_> = (0..regions.len())
                .map(|i| {
                    let name = format!("{i}.bin");
                    tempdir.path().join(name)
                })
                .collect();
            for (path, (data, _)) in section_files.iter().zip(regions.iter()) {
                log::debug!("Writing {} byte(s) to '{}'", data.len(), path.display());
                tokio::fs::write(&path, data)
                    .await
                    .map_err(RunError::CreateTempFile)?;
            }

            // Generate commands for `JLinkExe`
            let mut cmd = String::new();
            writeln!(cmd, "r").unwrap();
            for (path, (_, offset)) in section_files.iter().zip(regions.iter()) {
                writeln!(cmd, "loadbin \"{}\" {offset:#08x}", path.display()).unwrap();
            }
            writeln!(cmd, "setpc {entry:#x}").unwrap();
            writeln!(cmd, "g").unwrap();
            writeln!(cmd, "q").unwrap();

            // Flash the program and reset the chip
            // (`probe-rs` doesn't support FE310-based boards at this time)
            log::debug!("Launching JLinkExe and executing '{cmd:?}'");
            subprocess::CmdBuilder::new("JLinkExe")
                .args([
                    "-device",
                    "FE310",
                    "-if",
                    "JTAG",
                    "-speed",
                    "4000",
                    "-jtagconf",
                    "-1,-1",
                    "-autoconnect",
                    "1",
                    "-exitonerror",
                    "1",
                    "-nogui",
                    "1",
                ])
                .spawn_expecting_success_quiet_with_input(cmd.as_bytes())
                .await
                .map_err(RunError::Flash)?;

            log::debug!("Waiting for 1 seconds");

            // The stale RTT data from a previous run might still be there until
            // the new startup code zero-fills the memory.
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            log::debug!("Opening the debug probe using `probe-rs`");

            // Open the probe using `probe-rs`
            // (Actually, `JLinkExe` comes with their RTT host, but I'm too lazy
            // to check its usage)
            // TODO: Use the J-Link software for RTT connection
            let selector: probe_rs::DebugProbeSelector = "1366:1061".try_into().unwrap();
            let probe = probe_rs::Probe::open(selector).map_err(RunError::OpenProbe)?;

            let selector: probe_rs::config::TargetSelector = "riscv".try_into().unwrap();
            let session = Arc::new(AsyncMutex::new(
                probe
                    .attach(selector, probe_rs::Permissions::new())
                    .map_err(RunError::Attach)?,
            ));

            // Open the RTT channels
            Ok(super::probe_rs::attach_rtt(
                session.try_lock_arc().unwrap(),
                &exe,
                super::probe_rs::RttOptions {
                    // The RISC-V External Debug Support specification 0.13 (to
                    // which FE310 conforms) doesn't define any abstract command
                    // for memory access, so the hart should be halted every
                    // time we access RTT.
                    halt_on_access: true,
                },
            )
            .await?)
        })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ProcessElfError {
    #[error("Couldn't read the ELF file")]
    Read(#[source] std::io::Error),
    #[error("Couldn't parse the ELF file")]
    Parse(#[source] goblin::error::Error),
}

pub struct LoadableCode {
    /// The regions to be loaded onto the target.
    pub regions: Vec<(Vec<u8>, u64)>,
    /// The entry point.
    pub entry: u64,
}

/// Read the specified ELF file and return regions to be loaded onto the target.
pub async fn read_elf(exe: &Path) -> Result<LoadableCode, ProcessElfError> {
    let elf_bytes = tokio::fs::read(&exe).await.map_err(ProcessElfError::Read)?;

    spawn_blocking(move || {
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
    })
    .await
    .unwrap() // Ignore `JoinError`
}

struct OutputReader {
    child: Child,
}

impl AsyncRead for OutputReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(self.child.stdout.as_mut().unwrap()).poll_read(cx, buf)
    }
}
