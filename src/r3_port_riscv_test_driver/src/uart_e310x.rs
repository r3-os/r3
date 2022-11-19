//! The UART driver compatible with QEMU `sifive_e` machine
//! (RISC-V Board compatible with SiFive E SDK).
use core::fmt;
use e310x_hal::{
    e310x::{UART0, UART1},
    prelude::*,
    serial::{Serial, Tx, UartX},
    time::Bps,
};
use nb::block;
use riscv::interrupt;

static mut UART: Option<(Tx<UART0>, Tx<UART1>)> = None;

#[cold]
fn init() {
    let resources = unsafe { e310x_hal::DeviceResources::steal() };

    let clocks = super::e310x::clocks();

    let uart0 = Serial::new(
        resources.peripherals.UART0,
        (
            resources.pins.pin17.into_iof0(),
            resources.pins.pin16.into_iof0(),
        ),
        Bps(115200),
        clocks,
    );
    let (tx0, _rx) = uart0.split();

    let uart1 = Serial::new(
        resources.peripherals.UART1,
        (
            resources.pins.pin18.into_iof0(),
            resources.pins.pin23.into_iof0(),
        ),
        Bps(115200),
        clocks,
    );
    let (tx1, _rx) = uart1.split();

    unsafe { UART = Some((tx0, tx1)) };
}

#[inline]
fn with_uart(f: impl FnOnce(&mut (Tx<UART0>, Tx<UART1>))) {
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

pub fn stdout_write_str(s: &str) {
    with_uart(|(uart0, _uart1)| {
        let _ = SerialWrapper(uart0).write_str(s);
    });
}

pub fn stdout_write_fmt(args: fmt::Arguments<'_>) {
    with_uart(|(uart0, _uart1)| {
        let _ = SerialWrapper(uart0).write_fmt(args);
    });
}

pub fn stderr_write_fmt(args: fmt::Arguments<'_>) {
    with_uart(|(_uart0, uart1)| {
        let _ = SerialWrapper(uart1).write_fmt(args);
    });
}

struct SerialWrapper<'a, UART: UartX>(&'a mut Tx<UART>);

impl<UART: UartX> fmt::Write for SerialWrapper<'_, UART> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.as_bytes() {
            if *byte == b'\n' {
                let res = block!(self.0.write(b'\r'));

                if res.is_err() {
                    return Err(::core::fmt::Error);
                }
            }

            let res = block!(self.0.write(*byte));

            if res.is_err() {
                return Err(::core::fmt::Error);
            }
        }
        Ok(())
    }
}
