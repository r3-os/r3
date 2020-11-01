//! Serial driver
// This module is only intended to be used internally, hence the semver
// exemption. It probably should be in a HAL crate, but there's no HAL crate
// for RZ/A1.
#![cfg(feature = "semver-exempt")]
use super::gpio::{AltMode, GpioExt};
use core::convert::TryInto;

/// The extension trait for `rza1::SCIFx` (e.g., [`rza1::SCIF0`]).
///
/// # Safety
///
/// This is only meant to be implemented on `rza1::SCIFx`.
pub unsafe trait ScifExt:
    core::ops::Deref<Target = rza1::scif0::RegisterBlock> + Sized
{
    fn global() -> Self {
        assert_eq!(core::mem::size_of::<Self>(), 0);
        unsafe { core::mem::zeroed() }
    }

    fn configure_pins(&self, gpio: &rza1::gpio::RegisterBlock);

    fn enable_clock(&self, cpg: &rza1::cpg::RegisterBlock);

    #[inline]
    fn configure_uart(&self, baud_rate: u32) {
        self.scr.write(|w| {
            w.tie()
                .clear_bit()
                .rie()
                .clear_bit()
                .te()
                .set_bit()
                .re()
                .clear_bit()
                .reie()
                .clear_bit()
                .cke()
                .internal_sck_in()
        });
        self.smr.write(|w| {
            w
                // Asynchronous
                .ca()
                .clear_bit()
                // 8-bit data
                .chr()
                .clear_bit()
                // No parity bits
                .pe()
                .clear_bit()
                // One stop bit
                .stop()
                .clear_bit()
                .cks()
                .divide_by_1()
        });
        let brr: u8 = (2083333 / baud_rate - 1)
            .try_into()
            .expect("can't satisfy the baud rate specification");
        self.brr.write(|w| w.d().bits(brr));
    }

    fn into_nb_writer(self) -> NbWriter<Self> {
        NbWriter(self)
    }
}

unsafe impl ScifExt for rza1::SCIF2 {
    #[inline]
    fn configure_pins(&self, gpio: &rza1::gpio::RegisterBlock) {
        gpio.set_alt_mode((6, 2), AltMode::Alt7);
        gpio.set_alt_mode((6, 3), AltMode::Alt7);
        gpio.set_input((6, 2));
        gpio.set_output((6, 3));
    }

    #[inline]
    fn enable_clock(&self, cpg: &rza1::cpg::RegisterBlock) {
        cpg.stbcr4.modify(|_, w| w.mstp45().clear_bit());
    }
}

/// The adapter for [`ScifExt`] that uses [`::nb`] to notify the caller of a
/// blocking situation.
pub struct NbWriter<T>(T);

// Safety: `NbWriter` can do nothing with `&Self`, and its owner can't take a
//         reference to `self.0`
unsafe impl<T> Sync for NbWriter<T> {}

impl<T: ScifExt> embedded_hal::serial::Write<u8> for NbWriter<T> {
    type Error = core::convert::Infallible;

    fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
        let sc = &*self.0;
        if sc.fsr.read().tdfe().bit_is_set() {
            sc.ftdr.write(|w| w.d().bits(word));
            sc.fsr
                .modify(|_, w| w.tdfe().clear_bit().tend().clear_bit());
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        if self.0.fsr.read().tend().bit_is_set() {
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
}
