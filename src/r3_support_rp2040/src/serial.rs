//! Serial driver
// This module is only intended to be used internally, hence the semver
// exemption. It probably should be in a HAL crate.
#![cfg(feature = "semver-exempt")]

/// The extension trait for [`rp2040_pac::UART0`][].
///
/// # Safety
///
/// This is only meant to be implemented on `rp2040_pac::UART0`.
pub unsafe trait UartExt:
    core::ops::Deref<Target = rp2040_pac::uart0::RegisterBlock> + Sized
{
    fn global() -> Self {
        assert_eq!(core::mem::size_of::<Self>(), 0);
        unsafe { core::mem::zeroed() }
    }

    fn configure_pins(&self, io_bank0: &rp2040_pac::io_bank0::RegisterBlock);

    fn reset(&self, resets: &rp2040_pac::resets::RegisterBlock);

    #[inline]
    fn configure_uart(&self, baud_rate: u32) {
        let clk_peri = 48_000_000u32;
        let baud_rate_div = clk_peri.checked_mul(8).unwrap() / baud_rate;
        let baud_ibrd = baud_rate_div >> 7;
        let baud_fbrd = ((baud_rate_div & 0x7f) + 1) / 2;
        assert!(baud_ibrd > 0 && baud_ibrd < 65536);

        // Load PL011's baud divisor registers
        self.uartibrd.write(|b| unsafe { b.bits(baud_ibrd) });
        if baud_ibrd == 65535 {
            self.uartfbrd.write(|b| unsafe { b.bits(0) });
        } else {
            self.uartfbrd.write(|b| unsafe { b.bits(baud_fbrd) });
        }

        // PL011 needs a (dummy) line control register write to latch in the
        // divisors. We don't want to actually change LCR contents here.
        self.uartlcr_h.modify(|_, w| w);

        // Enable transmission, enable UART
        self.uartcr.write(|b| b.uarten().set_bit().txe().set_bit());

        // 8-bit words, enable FIFO
        self.uartlcr_h
            .write(|b| unsafe { b.wlen().bits(0b11).fen().set_bit() });
    }

    fn into_nb_writer(self) -> NbWriter<Self> {
        NbWriter(self)
    }
}

unsafe impl UartExt for rp2040_pac::UART0 {
    #[inline]
    fn configure_pins(&self, io_bank0: &rp2040_pac::io_bank0::RegisterBlock) {
        // GPIO0 → UART0 TX (F2)
        // GPIO1 → UART0 RX (F2)
        io_bank0.gpio[0]
            .gpio_ctrl
            .write(|b| unsafe { b.funcsel().bits(2) });
        io_bank0.gpio[1]
            .gpio_ctrl
            .write(|b| unsafe { b.funcsel().bits(2) });
    }

    fn reset(&self, resets: &rp2040_pac::resets::RegisterBlock) {
        resets.reset.modify(|_, w| w.uart0().set_bit());
        resets.reset.modify(|_, w| w.uart0().clear_bit());
        while resets.reset_done.read().uart0().bit_is_clear() {}
    }
}

/// The adapter for [`UartExt`] that uses [`::nb`] to notify the caller of a
/// blocking situation.
pub struct NbWriter<T>(T);

// Safety: `NbWriter` can do nothing with `&Self`, and its owner can't take a
//         reference to `self.0`
unsafe impl<T> Sync for NbWriter<T> {}

impl<T: UartExt> embedded_hal::serial::Write<u8> for NbWriter<T> {
    type Error = core::convert::Infallible;

    fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
        let uart = &*self.0;
        if uart.uartfr.read().txff().bit_is_set() {
            Err(nb::Error::WouldBlock)
        } else {
            uart.uartdr.write(|w| unsafe { w.data().bits(word) });
            Ok(())
        }
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        if self.0.uartfr.read().txfe().bit_is_set() {
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
}
