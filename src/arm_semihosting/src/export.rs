//! IMPLEMENTATION DETAILS USED BY MACROS

use core::fmt::{self, Write};

use crate::hio::{self, HStderr, HStdout};

static mut HSTDOUT: Option<HStdout> = None;

#[cfg(arm)]
fn interrupt_free<R>(f: impl FnOnce() -> R) -> R {
    let cpsr_old: u32;
    unsafe { llvm_asm!("mrs $0, cpsr":"=r"(cpsr_old):::"volatile") };
    unsafe { llvm_asm!("cpsid i"::::"volatile") };

    let ret = f();

    if cpsr_old & 0x80 == 0 {
        unsafe { llvm_asm!("cpsie i"::::"volatile") };
    }

    ret
}

#[cfg(not(arm))]
fn interrupt_free<R>(_: impl FnOnce() -> R) -> R {
    unreachable!();
}

pub fn hstdout_str(s: &str) -> Result<(), ()> {
    interrupt_free(|| unsafe {
        if HSTDOUT.is_none() {
            HSTDOUT = Some(hio::hstdout()?);
        }

        HSTDOUT.as_mut().unwrap().write_str(s).map_err(drop)
    })
}

pub fn hstdout_fmt(args: fmt::Arguments) -> Result<(), ()> {
    interrupt_free(|| unsafe {
        if HSTDOUT.is_none() {
            HSTDOUT = Some(hio::hstdout()?);
        }

        HSTDOUT.as_mut().unwrap().write_fmt(args).map_err(drop)
    })
}

static mut HSTDERR: Option<HStderr> = None;

pub fn hstderr_str(s: &str) -> Result<(), ()> {
    interrupt_free(|| unsafe {
        if HSTDERR.is_none() {
            HSTDERR = Some(hio::hstderr()?);
        }

        HSTDERR.as_mut().unwrap().write_str(s).map_err(drop)
    })
}

pub fn hstderr_fmt(args: fmt::Arguments) -> Result<(), ()> {
    interrupt_free(|| unsafe {
        if HSTDERR.is_none() {
            HSTDERR = Some(hio::hstderr()?);
        }

        HSTDERR.as_mut().unwrap().write_fmt(args).map_err(drop)
    })
}
