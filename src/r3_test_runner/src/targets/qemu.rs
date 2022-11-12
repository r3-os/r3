use anyhow::Result;
use std::{
    future::Future,
    io,
    path::Path,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, ReadBuf},
    process::Child,
};

use super::{DebugProbe, DynAsyncRead};
use crate::subprocess;

pub mod arm;
pub mod riscv;

struct QemuDebugProbe {
    qemu_cmd: &'static str,
    qemu_args: &'static [&'static str],
}

impl QemuDebugProbe {
    fn new(qemu_cmd: &'static str, qemu_args: &'static [&'static str]) -> Self {
        Self {
            qemu_cmd,
            qemu_args,
        }
    }
}

impl DebugProbe for QemuDebugProbe {
    fn program_and_get_output(
        &mut self,
        exe: &Path,
    ) -> Pin<Box<dyn Future<Output = Result<DynAsyncRead<'_>>> + '_>> {
        let result = subprocess::CmdBuilder::new(self.qemu_cmd)
            .arg("-kernel")
            .arg(exe)
            .args(self.qemu_args)
            .args([
                "-nographic",
                "-d",
                "guest_errors",
                "-audiodev",
                "id=none,driver=none",
            ])
            .spawn_and_get_child()
            .map(|child| Box::pin(OutputReader { child }) as DynAsyncRead<'static>)
            .map_err(|e| e.into());

        Box::pin(std::future::ready(result))
    }
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
