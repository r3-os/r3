use anyhow::Result;
use std::{
    convert::TryInto,
    fmt::Write,
    future::Future,
    io,
    path::Path,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};
use tempdir::TempDir;
use tokio::{io::AsyncRead, process::Child};

use super::{DebugProbe, DynAsyncRead, Target};
use crate::subprocess;

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
            let tempdir = TempDir::new("constance_test_runner").map_err(RunError::CreateTempDir)?;
            let section_files: Vec<_> = (0..regions.len())
                .map(|i| {
                    let name = format!("{}.bin", i);
                    tempdir.path().join(&name)
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
                writeln!(cmd, "loadbin \"{}\" 0x{:08x}", path.display(), offset).unwrap();
            }
            writeln!(cmd, "setpc 0x{:x}", entry).unwrap();
            writeln!(cmd, "g").unwrap();
            writeln!(cmd, "q").unwrap();

            // Flash the program and reset the chip
            // (`probe-rs` doesn't support FE310-based boards at this time)
            log::debug!("Launching JLinkExe and executing '{:?}'", cmd);
            subprocess::CmdBuilder::new("JLinkExe")
                .args(&[
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
            tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
            log::debug!("Opening the debug probe using `probe-rs`");

            // Open the probe using `probe-rs`
            // (Actually, `JLinkExe` comes with their RTT host, but I'm too lazy
            // to check its usage)
            // TODO: Use the J-Link software for RTT connection
            let selector: probe_rs::DebugProbeSelector = "1366:1061".try_into().unwrap();
            let probe = probe_rs::Probe::open(selector).map_err(RunError::OpenProbe)?;

            let selector: probe_rs::config::TargetSelector = "riscv".try_into().unwrap();
            let session = Arc::new(Mutex::new(
                probe.attach(selector).map_err(RunError::Attach)?,
            ));

            // Open the RTT channels
            Ok(super::probe_rs::attach_rtt(
                session,
                &exe,
                super::probe_rs::RttOptions {
                    // The RISC-V External Debug Support specification 0.13 (to
                    // which FE310 conforms) doesn't define any abstract command
                    // for memory access, so the hart should be halted every
                    // time we access RTT.
                    halt_on_access: true,
                    ..Default::default()
                },
            )
            .await?)
        })
    }
}

#[derive(thiserror::Error, Debug)]
enum ProcessElfError {
    #[error("Couldn't read the ELF file")]
    Read(#[source] std::io::Error),
    #[error("Couldn't parse the ELF file")]
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

struct OutputReader {
    child: Child,
}

impl AsyncRead for OutputReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(self.child.stdout.as_mut().unwrap()).poll_read(cx, buf)
    }
}
