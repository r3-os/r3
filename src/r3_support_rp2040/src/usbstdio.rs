//! Standard input/output, behaving as a USB serial device
// This module is only intended to be used internally, hence the semver
// exemption. It probably should be in a HAL or board crate.
#![cfg(feature = "semver-exempt")]
use crate::usb::UsbBus;
use core::cell::RefCell;
use cortex_m::{interrupt, singleton};
use r3::kernel::{
    cfg::CfgBuilder, InterruptHandler, InterruptLine, InterruptNum, Kernel, StartupHook,
};
use usb_device::{
    bus::UsbBusAllocator,
    device::{UsbDevice, UsbDeviceBuilder, UsbVidPid},
};
use usbd_serial::{SerialPort, USB_CLASS_CDC};

struct UsbStdioGlobal {
    usb_device: UsbDevice<'static, UsbBus>,
    serial: SerialPort<'static, UsbBus>,
    /// Yet another write buffer. We use this to withhold the data transmission
    /// until DTR (Data Terminal Ready) is asserted on the host side.
    write_buf: Deque<u8, 2048>,
}

static USB_STDIO_GLOBAL: interrupt::Mutex<RefCell<Option<UsbStdioGlobal>>> =
    interrupt::Mutex::new(RefCell::new(None));

/// Start a no-interrupt section and get the global instance of
/// `UsbStdioGlobal`. Will panic if the `UsbStdioGlobal` hasn't been initialized
/// yet.
fn with_usb_stdio_global<T>(f: impl FnOnce(&mut UsbStdioGlobal) -> T) -> T {
    interrupt::free(|cs| {
        let mut g = USB_STDIO_GLOBAL.borrow(cs).borrow_mut();
        let g = g
            .as_mut()
            .expect("UsbStdioGlobal hasn't been initialized yet");
        f(g)
    })
}

pub trait Options {
    fn handle_input(_s: &[u8]) {}
}

/// Add a USB serial device to the system and register it as the destination of
/// the standard output ([`crate::stdout`]).
pub const fn configure<System: Kernel + Options>(b: &mut CfgBuilder<System>) {
    StartupHook::build()
        .start(|_| {
            let p = unsafe { rp2040::Peripherals::steal() };

            // Reset PLL
            p.RESETS.reset.modify(|_, w| w.usbctrl().set_bit());
            p.RESETS.reset.modify(|_, w| w.usbctrl().clear_bit());
            while p.RESETS.reset_done.read().usbctrl().bit_is_clear() {}

            // Construct `UsbBusAllocator`. Since startup hooks are called only
            // once, this `singleton!` will succeed
            let usb_bus_allocator = singleton!(
                : UsbBusAllocator<UsbBus> =
                    UsbBusAllocator::new(UsbBus::new(p.USBCTRL_REGS))
            )
            .unwrap();

            // Construct a `SerialPort` associated with `usb_bus_allocator`
            let serial = SerialPort::new(usb_bus_allocator);

            // Construct a `UsbDeviceBuilder` associated with `usb_bus_allocator`
            let usb_device = UsbDeviceBuilder::new(usb_bus_allocator, UsbVidPid(0x16c0, 0x27dd))
                .product("r3_support_rp2040 standard I/O")
                .device_class(USB_CLASS_CDC)
                .max_packet_size_0(64)
                .build();

            interrupt::free(|cs| {
                *USB_STDIO_GLOBAL.borrow(cs).borrow_mut() = Some(UsbStdioGlobal {
                    serial,
                    usb_device,
                    write_buf: Deque::new(),
                })
            });

            // Register the standard output
            crate::stdout::set_stdout(NbWriter);
        })
        .finish(b);

    let int_num =
        rp2040::Interrupt::USBCTRL_IRQ as InterruptNum + r3_port_arm_m::INTERRUPT_EXTERNAL0;

    InterruptLine::build()
        .line(int_num)
        .priority(4) // meh
        .enabled(true)
        .finish(b);

    InterruptHandler::build()
        .line(int_num)
        .start(|_| {
            let mut buf = [0; 64];
            let mut read_len = 0;

            // Get the global `UsbStdioGlobal` instance, which should
            // have been created by the startup hook above
            with_usb_stdio_global(|g| {
                g.usb_device.poll(&mut [&mut g.serial]);

                if let Ok(len) = g.serial.read(&mut buf) {
                    read_len = len;
                }

                g.try_flush();
            });

            if read_len > 0 {
                System::handle_input(&buf[..read_len]);
            }
        })
        .finish(b);
}

struct NbWriter;

fn map_usb_error_to_nb_error(e: usb_device::UsbError) -> nb::Error<core::convert::Infallible> {
    match e {
        usb_device::UsbError::WouldBlock => nb::Error::WouldBlock,
        usb_device::UsbError::BufferOverflow
        | usb_device::UsbError::EndpointOverflow
        | usb_device::UsbError::Unsupported
        | usb_device::UsbError::InvalidEndpoint
        | usb_device::UsbError::EndpointMemoryOverflow => unreachable!("{:?}", e),
        // I think the following ones are protocol errors? I'm not sure
        // if they can be returned by `write` and `flush`.
        //
        // It's really a bad idea to gather all error codes in a single `enum`
        // without meticulously documenting how and when each of them will be
        // returned.
        usb_device::UsbError::ParseError | usb_device::UsbError::InvalidState => {
            panic!("{:?} is probably unexpected, but I'm not sure", e)
        }
    }
}

impl embedded_hal::serial::Write<u8> for NbWriter {
    type Error = core::convert::Infallible;

    fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
        with_usb_stdio_global(|g| {
            // Push the given byte to the write buffer. Return `WouldBlock` if
            // the buffer is full.
            g.write_buf.push(word).map_err(|_| nb::Error::WouldBlock)?;
            g.try_flush();
            Ok(())
        })
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        with_usb_stdio_global(|g| {
            g.try_flush();
            g.serial.flush().map_err(map_usb_error_to_nb_error)?;
            if !g.write_buf.is_empty() {
                return Err(nb::Error::WouldBlock);
            }
            Ok(())
        })
    }
}

impl UsbStdioGlobal {
    fn try_flush(&mut self) {
        // Withhold the data until DTR is asserted and RTS is cleared
        if !self.serial.dtr() {
            return;
        }

        let first_contiguous_bytes = self.write_buf.first_contiguous_slice();
        if !first_contiguous_bytes.is_empty() {
            match self
                .serial
                .write(first_contiguous_bytes)
                .map_err(map_usb_error_to_nb_error)
            {
                Ok(num_bytes) => {
                    self.write_buf.consume(num_bytes);
                }
                Err(nb::Error::WouldBlock) => {}
                // FIXME: `Infallible` is uninhabited, so this arm is really unreachable
                Err(nb::Error::Other(_)) => unreachable!(),
            }
        }
    }
}

// TODO:
//
//   - The USB controller needs to be periodically polled to work correctly.
//     The panic handler should poll USB instead of doing nothing.
//
//   - We also need to handle incoming data. The test driver will need this to
//     hold off the test execution until requested and to prepare the target for
//     a subsequent test run.
//
//   - If there's incoming data, the interrupt will not be deassserted until
//     `SerialPort::read` is called. And `SerialPort::read` does not consume
//     the incoming data if its internal buffer is full.
//
//     The only way to do flow control seems to be disabling or ignoring USB
//     interrupts. Of course, this can only be done for a few milliseconds, the
//     upper bound defined by the USB specification.
//

struct Deque<T, const LEN: usize> {
    buf: [T; LEN],
    start: usize,
    len: usize,
}

impl<T: r3::utils::Init + Copy, const LEN: usize> Deque<T, LEN> {
    #[inline]
    const fn new() -> Self {
        Self {
            buf: [T::INIT; LEN],
            start: 0,
            len: 0,
        }
    }

    #[inline]
    fn first_contiguous_slice(&self) -> &[T] {
        let s = &self.buf[self.start..];
        if s.len() >= self.len {
            &s[..self.len]
        } else {
            s
        }
    }

    /// Remove the specified number of elements from the beginning.
    #[inline]
    fn consume(&mut self, count: usize) {
        debug_assert!(count <= self.len);
        self.len -= count;
        self.start = (self.start + count) % self.buf.len();
    }

    #[inline]
    fn push(&mut self, x: T) -> Result<(), ()> {
        if self.len >= self.buf.len() {
            Err(())
        } else {
            self.buf[(self.start + self.len) % self.buf.len()] = x;
            self.len += 1;
            Ok(())
        }
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Append the specified slice to the end of `self`. Returns the number of
    /// added elements.
    #[allow(dead_code)]
    fn extend_from_slice(&mut self, mut src: &[T]) -> usize {
        // Cap by the remaining capacity
        src = &src[..src.len().min(self.buf.len() - self.len)];

        // Copy the first part
        let end = (self.start + self.len) % self.buf.len();
        self.start = end;
        let dst1 = &mut self.buf[end..];
        if src.len() > dst1.len() {
            dst1.copy_from_slice(&src[..dst1.len()]);
            src = &src[dst1.len()..];
        } else {
            dst1[..src.len()].copy_from_slice(src);
            return src.len();
        }

        // Copy the second part
        let dst2 = &mut self.buf[..src.len()];
        dst2.copy_from_slice(src);

        src.len()
    }
}
