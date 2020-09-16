//! Clock initialization for Kendryte K210-based boards
use k210_hal::clock::Clocks;
use riscv::interrupt;

static mut CLOCKS: Option<Clocks> = None;

pub const MTIME_FREQUENCY: u64 = 10_000_000; // TODO

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
    let clocks = Clocks::new();
    unsafe { CLOCKS = Some(clocks) };
}
