//! Clock initialization for HiFive1, Red-V
use e310x_hal::{
    clock::Clocks,
    prelude::*,
    time::{Bps, Hertz},
};
use riscv::interrupt;

static mut CLOCKS: Option<Clocks> = None;

#[cfg(feature = "board-e310x-red-v")]
pub const MTIME_FREQUENCY: u64 = 32768;
#[cfg(feature = "board-e310x-qemu")]
pub const MTIME_FREQUENCY: u64 = 10_000_000;

#[inline]
pub fn clocks() -> Clocks {
    interrupt::free(
        #[inline]
        |_| unsafe {
            if CLOCKS.is_none() {
                init();
            }
            CLOCKS.unwrap_or_else(|| core::hint::unreachable_unchecked())
        },
    )
}

#[cold]
fn init() {
    let resources = unsafe { e310x_hal::DeviceResources::steal() };

    let coreclk = resources
        .peripherals
        .PRCI
        .constrain()
        .use_external(Hertz(16_000_000))
        .coreclk(Hertz(150_000_000));

    let aonclk = resources
        .peripherals
        .AONCLK
        .constrain()
        .use_external(Hertz(32_768));

    let clocks = Clocks::freeze(coreclk, aonclk);
    unsafe { CLOCKS = Some(clocks) };
}
