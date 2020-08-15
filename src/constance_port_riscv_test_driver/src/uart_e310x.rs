//! The UART driver compatible with QEMU `sifive_e` machine
//! (RISC-V Board compatible with SiFive E SDK).
use core::fmt;
use e310x_hal::{
    clock::Clocks,
    e310x::{UART0, UART1},
    prelude::*,
    serial::{Serial, Tx, UartX},
    time::{Bps, Hertz},
};
use nb::block;

static mut UART0: Option<Tx<UART0>> = None;
static mut UART1: Option<Tx<UART1>> = None;

pub fn init() {
    let resources = unsafe { e310x_hal::DeviceResources::steal() };

    let coreclk = resources
        .peripherals
        .PRCI
        .constrain()
        .use_external(Hertz(16_000_000))
        .coreclk(Hertz(16_000_000));

    let aonclk = resources
        .peripherals
        .AONCLK
        .constrain()
        .use_external(Hertz(32_768));

    let clocks = Clocks::freeze(coreclk, aonclk);

    let uart0 = Serial::new(
        resources.peripherals.UART0,
        (
            resources.pins.pin17.into_iof0(),
            resources.pins.pin16.into_iof0(),
        ),
        Bps(115200),
        clocks,
    );
    let (tx, _rx) = uart0.split();
    unsafe { UART0 = Some(tx) };

    let uart1 = Serial::new(
        resources.peripherals.UART1,
        (
            resources.pins.pin18.into_iof0(),
            resources.pins.pin23.into_iof0(),
        ),
        Bps(115200),
        clocks,
    );
    let (tx, _rx) = uart1.split();
    unsafe { UART1 = Some(tx) };
}

/// Open the standard output channel (used for reporting results).
pub fn stdout() -> impl fmt::Write {
    SerialWrapper(unsafe { UART0.as_mut() }.unwrap())
}

/// Open the standard error channel (used for logging).
pub fn stderr() -> impl fmt::Write {
    SerialWrapper(unsafe { UART1.as_mut() }.unwrap())
}

struct SerialWrapper<UART: UartX + 'static>(&'static mut Tx<UART>);

impl<UART: UartX + 'static> fmt::Write for SerialWrapper<UART> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.as_bytes() {
            if *byte == '\n' as u8 {
                let res = block!(self.0.write('\r' as u8));

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
