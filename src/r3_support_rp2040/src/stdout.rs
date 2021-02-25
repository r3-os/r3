//! Standard output
//!
//! Warning: It can block the calling thread by polling. It will use
//! [`cortex_m::interrupt:free`] to enter a critical section, which can be cut
//! short if the destination gets stuck.
// This module is only intended to be used internally, hence the semver
// exemption. It probably should be in a HAL crate.
#![cfg(feature = "semver-exempt")]
use core::{cell::RefCell, convert::Infallible, fmt};
use cortex_m::interrupt;
use inline_dyn::{inline_dyn, InlineDyn};
use nb::block;

pub fn set_stdout(writer: impl SerialWrite) {
    interrupt::free(|cs| {
        *STDOUT.borrow(cs).borrow_mut() = Some(inline_dyn![SerialWrite; writer].ok().unwrap());
    });
}

pub trait SerialWrite:
    embedded_hal::serial::Write<u8, Error = Infallible> + Send + Sync + 'static
{
}
impl<T> SerialWrite for T where
    T: embedded_hal::serial::Write<u8, Error = Infallible> + Send + Sync + 'static
{
}

type InlineDynWrite = InlineDyn<'static, dyn SerialWrite>;

static STDOUT: interrupt::Mutex<RefCell<Option<InlineDynWrite>>> =
    interrupt::Mutex::new(RefCell::new(None));

struct WrapSerialWrite;

impl WrapSerialWrite {
    fn write_bytes_inner(mut s: &[u8]) -> core::fmt::Result {
        loop {
            if s.is_empty() {
                break Ok(());
            }

            block!(interrupt::free(|cs| -> nb::Result<(), core::fmt::Error> {
                let mut stdout = STDOUT.borrow(cs).borrow_mut();
                if let Some(stdout) = &mut *stdout {
                    loop {
                        match s {
                            [] => {
                                break Ok(());
                            }
                            [head, tail @ ..] => {
                                // Output the first byte. If this gets stuck,
                                // break out of `interrupt::free`.
                                stdout
                                    .write(*head)
                                    .map_err(|e| e.map(|_| core::fmt::Error))?;
                                s = tail;
                            }
                        }
                    }
                } else {
                    Ok(())
                }
            }))?;
        }
    }

    fn write_bytes(mut s: &[u8]) -> core::fmt::Result {
        while let Some(i) = s.iter().position(|&x| x == b'\n') {
            if i > 0 {
                Self::write_bytes_inner(&s[0..i])?;
            }

            Self::write_bytes_inner(b"\r\n")?;

            s = &s[i + 1..];
        }
        if s.len() > 0 {
            Self::write_bytes_inner(s)?;
        }
        Ok(())
    }
}

impl core::fmt::Write for WrapSerialWrite {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        Self::write_bytes(s.as_bytes())
    }
}

pub fn write_bytes(s: &[u8]) {
    let _ = WrapSerialWrite::write_bytes(s);
}

#[doc(hidden)]
pub fn write_str(s: &str) {
    let _ = fmt::Write::write_str(&mut WrapSerialWrite, s);
}

#[doc(hidden)]
pub fn write_fmt(args: fmt::Arguments<'_>) {
    let _ = fmt::Write::write_fmt(&mut WrapSerialWrite, args);
}

/// Macro for printing to the serial standard output
#[macro_export]
macro_rules! sprint {
    ($s:expr) => {
        $crate::stdout::write_str($s)
    };
    ($($tt:tt)*) => {
        $crate::stdout::write_fmt(format_args!($($tt)*))
    };
}

/// Macro for printing to the serial standard output, with a newline.
#[macro_export]
macro_rules! sprintln {
    () => {
        $crate::stdout::write_str("\n")
    };
    ($s:expr) => {
        $crate::stdout::write_str(concat!($s, "\n"))
    };
    ($s:expr, $($tt:tt)*) => {
        $crate::stdout::write_fmt(format_args!(concat!($s, "\n"), $($tt)*))
    };
}
