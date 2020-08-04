use std::{
    error::Error,
    future::Future,
    io,
    path::Path,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{io::AsyncRead, process::Child};

use super::{DebugProbe, DynAsyncRead};
use crate::subprocess;

pub(super) struct QemuDebugProbe {
    qemu_args: &'static [&'static str],
}

impl QemuDebugProbe {
    pub(super) fn new(qemu_args: &'static [&'static str]) -> Self {
        Self { qemu_args }
    }
}

impl DebugProbe for QemuDebugProbe {
    fn program_and_get_output(
        &mut self,
        exe: &Path,
    ) -> Pin<Box<dyn Future<Output = Result<DynAsyncRead<'_>, Box<dyn Error>>> + '_>> {
        let result = subprocess::CmdBuilder::new("qemu-system-arm")
            .arg("-kernel")
            .arg(exe)
            .args(self.qemu_args)
            .args(&[
                "-nographic",
                "-d",
                "guest_errors",
                "-semihosting",
                "-semihosting-config",
                "target=native",
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
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(self.child.stdout.as_mut().unwrap()).poll_read(cx, buf)
    }
}
