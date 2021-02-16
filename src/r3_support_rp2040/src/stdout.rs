//! Standard output
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

/// `WrapSerialWrite` implements the [`core::fmt::Write`] trait for
/// [`embedded_hal::serial::Write`] implementations.
struct WrapSerialWrite<'p, T: ?Sized>(&'p mut T);

impl<'p, T: ?Sized> core::fmt::Write for WrapSerialWrite<'p, T>
where
    T: embedded_hal::serial::Write<u8>,
{
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.as_bytes() {
            if *byte == b'\n' {
                let res = block!(self.0.write(b'\r'));

                if res.is_err() {
                    return Err(core::fmt::Error);
                }
            }

            let res = block!(self.0.write(*byte));

            if res.is_err() {
                return Err(core::fmt::Error);
            }
        }
        Ok(())
    }
}

#[doc(hidden)]
pub fn write_str(s: &str) {
    interrupt::free(|cs| {
        let mut stdout = STDOUT.borrow(cs).borrow_mut();
        if let Some(stdout) = &mut *stdout {
            let _ = fmt::Write::write_str(&mut WrapSerialWrite(&mut **stdout), s);
        }
    })
}

#[doc(hidden)]
pub fn write_fmt(args: fmt::Arguments<'_>) {
    interrupt::free(|cs| {
        let mut stdout = STDOUT.borrow(cs).borrow_mut();
        if let Some(stdout) = &mut *stdout {
            let _ = fmt::Write::write_fmt(&mut WrapSerialWrite(&mut **stdout), args);
        }
    })
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
