//! SLIP (Serial Line Internet Protocol)
use futures_core::ready;
use std::{
    future::Future,
    marker::Unpin,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncBufRead, AsyncWrite, AsyncWriteExt};

struct FrameExtractorState {
    inner: FrameExtractorStateInner,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FrameExtractorStateInner {
    Initial,
    Frame,
    FrameEscape,
}

#[derive(Debug, Clone, Copy)]
#[expect(clippy::enum_variant_names)] // The common suffix `Frame` is intentional
enum FrameExtractorAction {
    /// Clear the partial packet buffer.
    StartFrame,
    /// Append a byte to the partial packet buffer.
    AppendFrame(u8),
    /// Process the contents of the partial buffer as a complete packet.
    EndFrame,
}

#[derive(thiserror::Error, Debug, Clone, Copy)]
pub enum FrameExtractorProtocolError {
    #[error("Expected SLIP escape, got 0x{0:x}")]
    InvalidEscape(u8),
}

impl FrameExtractorState {
    fn new() -> Self {
        Self {
            inner: FrameExtractorStateInner::Initial,
        }
    }

    fn process(
        &mut self,
        b: u8,
    ) -> Result<Option<FrameExtractorAction>, FrameExtractorProtocolError> {
        match self.inner {
            FrameExtractorStateInner::Initial => match b {
                0xc0 => {
                    self.inner = FrameExtractorStateInner::Frame;
                    Ok(Some(FrameExtractorAction::StartFrame))
                }
                _ => {
                    log::trace!("Ignoring 0x{:?} outside a frame", b);
                    Ok(None)
                }
            },
            FrameExtractorStateInner::Frame => match b {
                0xdb => {
                    self.inner = FrameExtractorStateInner::FrameEscape;
                    Ok(None)
                }
                0xc0 => {
                    self.inner = FrameExtractorStateInner::Initial;
                    Ok(Some(FrameExtractorAction::EndFrame))
                }
                _ => Ok(Some(FrameExtractorAction::AppendFrame(b))),
            },
            FrameExtractorStateInner::FrameEscape => {
                self.inner = FrameExtractorStateInner::Frame;
                match b {
                    0xdc => Ok(Some(FrameExtractorAction::AppendFrame(0xc0))),
                    0xdd => Ok(Some(FrameExtractorAction::AppendFrame(0xdb))),
                    _ => Err(FrameExtractorProtocolError::InvalidEscape(b)),
                }
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum FrameExtractorError {
    #[error("Protocol error")]
    Protocol(#[source] FrameExtractorProtocolError),
    #[error("I/O error")]
    Io(#[source] std::io::Error),
}

pub fn read_frame<T: AsyncBufRead + Unpin>(reader: &mut T) -> ReadFrame<'_, T> {
    ReadFrame {
        reader,
        partial_packet: Vec::new(),
        state: FrameExtractorState::new(),
    }
}

pub struct ReadFrame<'a, T> {
    reader: &'a mut T,
    partial_packet: Vec<u8>,
    state: FrameExtractorState,
}

impl<T: AsyncBufRead + Unpin> Future for ReadFrame<'_, T> {
    type Output = Result<Vec<u8>, FrameExtractorError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = Pin::into_inner(self);
        let Self {
            reader,
            partial_packet,
            state,
        } = this;
        let mut consumed = 0;

        let result = 'result: loop {
            let buffer = match ready!(Pin::new(&mut *reader).poll_fill_buf(cx)) {
                Ok(x) => x,
                Err(e) => return Poll::Ready(Err(FrameExtractorError::Io(e))),
            };

            for &b in buffer {
                consumed += 1;

                match state.process(b) {
                    Ok(Some(FrameExtractorAction::StartFrame)) => {
                        partial_packet.clear();
                    }
                    Ok(Some(FrameExtractorAction::AppendFrame(b))) => {
                        partial_packet.push(b);
                    }
                    Ok(Some(FrameExtractorAction::EndFrame)) => {
                        break 'result Ok(std::mem::take(partial_packet));
                    }
                    Ok(None) => {}
                    Err(e) => {
                        break 'result Err(FrameExtractorError::Protocol(e));
                    }
                }
            }

            Pin::new(&mut *reader).consume(consumed);
            consumed = 0;
        };

        Pin::new(&mut *reader).consume(consumed);

        Poll::Ready(result)
    }
}

/// Apply SLIP framing to `data`, appending the result to `out`.
fn escape_frame(data: &[u8], out: &mut Vec<u8>) {
    let extra_len = data.iter().filter(|x| matches!(x, 0xdb | 0xc0)).count();
    out.reserve(data.len() + extra_len + 2);
    out.push(0xc0);
    for &b in data {
        match b {
            0xdb => {
                out.push(0xdb);
                out.push(0xdd);
            }
            0xc0 => {
                out.push(0xdb);
                out.push(0xdc);
            }
            _ => out.push(b),
        }
    }
    out.push(0xc0);
}

pub async fn write_frame(
    writer: &mut (impl AsyncWrite + Unpin),
    data: &[u8],
) -> std::io::Result<()> {
    let mut buf = Vec::new();
    escape_frame(data, &mut buf);
    writer.write_all(&buf).await
}
