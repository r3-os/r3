//! Demultiplexes a single stream into a primary message channel (for converying
//! panic messages and test results) and a log channel.
//!
//! The input bytes are delimited by `0x17` ([ETB], [`DELIM`]). Each chunk
//! starts with a byte indicating the channel of the chunk.
//!
//!  - `b'1'` ([`CHANNEL_PRIMARY`]) indicates the chunk belongs to a primary
//!    message channel, which can be read through the `AsyncRead` interface of
//!    [`Demux`].
//!  - `b'2'` ([`CHANNEL_LOG`]) indicates the chunk belongs to a log channel,
//!    which will be redirected to stdout.
//!
//! [ETB]: https://en.wikipedia.org/wiki/End-of-Transmission-Block_character
use futures_core::ready;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, ReadBuf};

/// The demultiplexing adapter for [`AsyncBufRead`] types. See the module-level
/// documentation for details on the multiplexing protocol that this adapter
/// type conforms with.
pub struct Demux<'a> {
    inner: Pin<Box<dyn AsyncBufRead + Send + 'a>>,
    st: State,
    log_out: tokio::io::Stdout,
}

#[derive(PartialEq)]
enum State {
    Header,
    Primary,
    Log,
}

const CHANNEL_PRIMARY: u8 = b'1';
const CHANNEL_LOG: u8 = b'2';
const DELIM: u8 = 0x17;

impl<'a> Demux<'a> {
    pub fn new(inner: impl AsyncBufRead + Send + 'a) -> Self {
        Demux {
            inner: Box::pin(inner),
            st: State::Header,
            log_out: tokio::io::stdout(),
        }
    }
}

impl AsyncRead for Demux<'_> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Naïve implementation of `poll_read` that uses `<Self as AsyncBufRead>`
        let my_buf = ready!(Pin::as_mut(&mut self).poll_fill_buf(cx))?;
        let num_bytes_read = my_buf.len().min(buf.remaining());
        buf.put_slice(&my_buf[..num_bytes_read]);
        Pin::as_mut(&mut self).consume(num_bytes_read);
        Poll::Ready(Ok(()))
    }
}

impl AsyncBufRead for Demux<'_> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        let this = Pin::into_inner(self);
        loop {
            let inner_buf = match ready!(Pin::new(&mut this.inner).poll_fill_buf(cx)) {
                Ok(x) => x,
                Err(e) => return Poll::Ready(Err(e)),
            };

            assert!(!inner_buf.is_empty());

            if this.st == State::Header {
                match inner_buf[0] {
                    CHANNEL_PRIMARY => {
                        this.st = State::Primary;
                        Pin::new(&mut this.inner).consume(1);
                    }
                    CHANNEL_LOG => {
                        this.st = State::Log;
                        Pin::new(&mut this.inner).consume(1);
                    }
                    DELIM => {
                        Pin::new(&mut this.inner).consume(1);
                    }
                    _ => {
                        // Protocol violation - assume this is a primary channel
                        this.st = State::Primary;
                    }
                }
                continue;
            }

            let payload_len = inner_buf
                .iter()
                .position(|b| *b == DELIM)
                .unwrap_or(inner_buf.len());

            if payload_len == 0 {
                // End of the chunk
                this.st = State::Header;
                Pin::new(&mut this.inner).consume(1);
                continue;
            }

            match this.st {
                State::Header => unreachable!(),
                State::Log => {
                    match ready!(
                        Pin::new(&mut this.log_out).poll_write(cx, &inner_buf[..payload_len])
                    ) {
                        Ok(num_bytes) => {
                            Pin::new(&mut this.inner).consume(num_bytes);
                        }
                        Err(e) => {
                            log::trace!("Ignoring error while outputting to stdout: {e:?}");
                            // Ignore any I/O errors caused by stdout
                            Pin::new(&mut this.inner).consume(payload_len);
                        }
                    }
                }
                State::Primary => {
                    // Expose the inner `AsyncBufRead` object's buffer to the
                    // caller.

                    // Re-borrow the inner buffer because the lifetime of this
                    // method's return type (`&[u8]`) must be identical to that
                    // of `self`. (`inner_buf` is derived through partial
                    // borrow of `this.inner`, so its lifetime is smaller than
                    // that of `self`.)
                    return Poll::Ready(
                        ready!(Pin::new(&mut this.inner).poll_fill_buf(cx))
                            .map(|inner_buf| &inner_buf[..payload_len]),
                    );
                }
            }
        }
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        Pin::new(&mut self.inner).consume(amt);
    }
}
