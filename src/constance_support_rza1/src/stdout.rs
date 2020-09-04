//! Standard output
// This module is only intended to be used internally, hence the semver
// exemption. It probably should be in a HAL crate, but there's no HAL crate
// for RZ/A1.
#![cfg(feature = "semver-exempt")]
use core::fmt;
use nb::block;

use crate::serial::{NbWriter, ScifExt};

pub fn set_stdout<T: ScifExt>(_writer: NbWriter<T>) {
    use core::fmt::Write;
    // We want to erase the type of `T`, but `static` can't store an unsized
    // owned value. `T: ScifExt` is guaranteed to be zero-sized, so we
    // conjure it up again out of thin air by calling `T::global()`.
    interrupt_free(|| unsafe {
        STDOUT = Some((
            |s| {
                let _ = Stdout(&mut T::global().into_nb_writer()).write_str(s);
            },
            |args| {
                let _ = Stdout(&mut T::global().into_nb_writer()).write_fmt(args);
            },
        ));
    })
}

static mut STDOUT: Option<(fn(&str), fn(fmt::Arguments<'_>))> = None;

/// `Stdout` implements the [`core::fmt::Write`] trait for
/// [`embedded_hal::serial::Write`] implementations.
struct Stdout<'p, T>(&'p mut T);

impl<'p, T> core::fmt::Write for Stdout<'p, T>
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

#[inline]
fn interrupt_free<T>(x: impl FnOnce() -> T) -> T {
    let cpsr: u32;
    unsafe { asm!("mrs {}, cpsr", out(reg)cpsr) };
    let unmask = (cpsr & (1 << 7)) == 0;

    unsafe { asm!("cpsid i") };

    let ret = x();

    if unmask {
        unsafe { asm!("cpsie i") };
    }

    ret
}

#[doc(hidden)]
pub fn write_str(s: &str) {
    interrupt_free(|| unsafe {
        if let Some(stdout) = STDOUT.as_ref() {
            (stdout.0)(s);
        }
    })
}

#[doc(hidden)]
pub fn write_fmt(args: fmt::Arguments<'_>) {
    interrupt_free(|| unsafe {
        if let Some(stdout) = STDOUT.as_ref() {
            (stdout.1)(args);
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
