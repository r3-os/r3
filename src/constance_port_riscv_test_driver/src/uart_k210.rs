//! The UART driver compatible with QEMU `sifive_e` machine
//! (RISC-V Board compatible with SiFive E SDK).
use core::fmt;
use k210_hal::{pac, prelude::*, serial::Serial, time::Bps};
use nb::block;
use riscv::interrupt;

static mut UART: Option<(fn(u8), fn(u8))> = None;

#[cold]
fn init() {
    let p = unsafe { k210_hal::Peripherals::steal() };

    let clocks = super::k210::clocks();

    let uart0 = p
        .UARTHS
        .configure((p.pins.pin5, p.pins.pin4), 115_200.bps(), &clocks);
    let (mut tx0, _rx) = uart0.split();

    let uart1 = p
        .UART1
        .configure((p.pins.pin6, p.pins.pin7), 115_200.bps(), &clocks);
    let (mut tx1, _rx) = uart1.split();

    unsafe {
        UART = Some((
            zst_closure(move |b| {
                let _ = block!(tx0.write(b));
            }),
            zst_closure(move |b| {
                let _ = block!(tx1.write(b));
            }),
        ))
    };
}

unsafe fn zst_closure<T: FnMut(u8)>(f: T) -> fn(u8) {
    assert_eq!(core::mem::size_of::<T>(), 0);
    |b| unsafe {
        let mut ctx = core::mem::MaybeUninit::<T>::uninit();
        (*ctx.as_mut_ptr())(b);
    }
}

#[inline]
fn with_uart(f: impl FnOnce(&mut (fn(u8), fn(u8)))) {
    interrupt::free(
        #[inline]
        |_| unsafe {
            if UART.is_none() {
                init();
            }
            f(UART
                .as_mut()
                .unwrap_or_else(|| core::hint::unreachable_unchecked()));
        },
    );
}

pub fn stdout_write_fmt(args: fmt::Arguments<'_>) {
    with_uart(|(uart0, _uart1)| {
        let _ = SerialWrapper(*uart0).write_fmt(args);
    });
}

pub fn stderr_write_fmt(args: fmt::Arguments<'_>) {
    with_uart(|(_uart0, uart1)| {
        let _ = SerialWrapper(*uart1).write_fmt(args);
    });
}

struct SerialWrapper(fn(u8));

impl fmt::Write for SerialWrapper {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.as_bytes() {
            if *byte == '\n' as u8 {
                (self.0)('\r' as u8);
            }

            (self.0)(*byte);
        }
        Ok(())
    }
}
