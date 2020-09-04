use std::{
    error::Error,
    future::Future,
    io,
    path::Path,
    pin::Pin,
    task::{Context, Poll},
};
use tempdir::TempDir;
use tokio::{io::AsyncRead, process::Child};

use super::{DebugProbe, DynAsyncRead};
use crate::subprocess;

#[derive(thiserror::Error, Debug)]
enum GrPeachOpenOcdDebugProbeGetOutputError {
    #[error("{0}")]
    ProcessElf(#[source] ProcessElfError),
    #[error("Error while creating a temporary directory: {0}")]
    CreateTempDir(#[source] std::io::Error),
    #[error("Error while creating a temporary file: {0}")]
    CreateTempFile(#[source] std::io::Error),
    #[error("Error while download the image: {0}")]
    Download(#[source] subprocess::SubprocessError),
    #[error("Error while running the program: {0}")]
    Run(#[source] subprocess::SubprocessError),
}

const GR_PEACH_INIT: &str = "
source [find interface/cmsis-dap.cfg]
source [find target/swj-dp.tcl]

set _CHIPNAME rza1
swj_newdap $_CHIPNAME cpu -expected-id 0x3ba02477

set _TARGETNAME $_CHIPNAME.cpu
target create $_TARGETNAME cortex_a -chain-position $_TARGETNAME

adapter_khz 1000
reset_config trst_and_srst
debug_level 0
init
halt
cortex_a dbginit

";

const GR_PEACH_RESET: &str = "
reset halt

# Enable writes to RAM
mwb 0xFCFE0400 0xff
mwb 0xFCFE0404 0xff
mwb 0xFCFE0408 0xff
";

pub(super) struct GrPeachOpenOcdDebugProbe {}

impl GrPeachOpenOcdDebugProbe {
    pub(super) fn new() -> Self {
        log::warn!(
            "this target doesn't support redirecting log output. use an
            external serial terminal program to see the log output"
        );
        Self {}
    }
}

impl DebugProbe for GrPeachOpenOcdDebugProbe {
    fn program_and_get_output(
        &mut self,
        exe: &Path,
    ) -> Pin<Box<dyn Future<Output = Result<DynAsyncRead<'_>, Box<dyn Error>>> + '_>> {
        let exe = exe.to_owned();
        let openocd_cmd = "openocd";
        Box::pin(async move {
            // Find the entry point
            let entry = entry_point_of_elf_file(&exe)
                .await
                .map_err(GrPeachOpenOcdDebugProbeGetOutputError::ProcessElf)?;

            let tempdir = TempDir::new("constance_test_runner")
                .map_err(GrPeachOpenOcdDebugProbeGetOutputError::CreateTempDir)?;

            // Download the image. Abort if there was any errors.
            let cmd_file = tempdir.path().join("download.cfg");
            tokio::fs::write(
                &cmd_file,
                format!(
                    "{}
                    {}
                    load_image \"{}\"
                    shutdown",
                    GR_PEACH_INIT,
                    GR_PEACH_RESET,
                    exe.display(),
                ),
            )
            .await
            .map_err(GrPeachOpenOcdDebugProbeGetOutputError::CreateTempFile)?;

            subprocess::CmdBuilder::new(openocd_cmd)
                .arg("-f")
                .arg(&cmd_file)
                .spawn_expecting_success_quiet()
                .await
                .map_err(GrPeachOpenOcdDebugProbeGetOutputError::Download)?;

            // Subscribe to the semihosting output and start the program
            let cmd_file = tempdir.path().join("run.cfg");
            tokio::fs::write(
                &cmd_file,
                format!(
                    "{}
                    arm semihosting enable
                    resume 0x{:x}",
                    GR_PEACH_INIT, entry,
                ),
            )
            .await
            .map_err(GrPeachOpenOcdDebugProbeGetOutputError::CreateTempFile)?;

            let log_file = tempdir.path().join("output.log");

            let result = subprocess::CmdBuilder::new(openocd_cmd)
                .arg("-f")
                .arg(&cmd_file)
                .arg("-l")
                .arg(&log_file)
                .spawn_and_get_child()
                .map(move |child| {
                    Box::pin(OutputReader {
                        child,
                        // Make sure `run.cfg` exists when OpenOCD reads it
                        _tempdir: tempdir,
                    }) as DynAsyncRead<'static>
                })
                .map_err(GrPeachOpenOcdDebugProbeGetOutputError::Run)?;

            Ok(result)
        })
    }
}

#[derive(thiserror::Error, Debug)]
enum ProcessElfError {
    #[error("Couldn't read the ELF file: {0}")]
    Read(#[source] std::io::Error),
    #[error("Couldn't parse the ELF file: {0}")]
    Parse(#[source] goblin::error::Error),
}

/// Read the specified ELF file and find the entry point.
async fn entry_point_of_elf_file(exe: &Path) -> Result<u64, ProcessElfError> {
    let elf_bytes = tokio::fs::read(&exe).await.map_err(ProcessElfError::Read)?;
    let elf = goblin::elf::Elf::parse(&elf_bytes).map_err(ProcessElfError::Parse)?;

    Ok(elf.entry)
}

struct OutputReader {
    child: Child,
    _tempdir: TempDir,
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
