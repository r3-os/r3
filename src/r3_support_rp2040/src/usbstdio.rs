//! Standard input/output, behaving as a USB serial device
// This module is only intended to be used internally, hence the semver
// exemption. It probably should be in a HAL or board crate.
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
}

static USB_STDIO_GLOBAL: interrupt::Mutex<RefCell<Option<UsbStdioGlobal>>> =
    interrupt::Mutex::new(RefCell::new(None));

/// Add a USB serial device to the system and register it as the destination of
/// the standard output ([`crate::stdout`]).
pub const fn configure<System: Kernel>(b: &mut CfgBuilder<System>) {
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
                *USB_STDIO_GLOBAL.borrow(cs).borrow_mut() =
                    Some(UsbStdioGlobal { serial, usb_device })
            });
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
            interrupt::free(|cs| {
                // Get the global `UsbStdioGlobal` instance, which should
                // have been created by the startup hook above
                let mut g = USB_STDIO_GLOBAL.borrow(cs).borrow_mut();
                let g = g.as_mut().unwrap();

                g.usb_device.poll(&mut [&mut g.serial]);

                let mut buf = [0; 64];
                if let Ok(len) = g.serial.read(&mut buf) {
                    if len > 0 {
                        // TEST
                        crate::sprintln!("{:?}", &buf[..len]);
                        if buf[..len] == b"\r"[..] || buf[..len] == b"\n"[..] {
                            let _ = g.serial.write(b"\r\n");
                            return;
                        }
                        let _ = g.serial.write(&[b'[']);
                        let _ = g.serial.write(&buf[..len]);
                        let _ = g.serial.write(&[b']']);
                    }
                }
            });
        })
        .finish(b);
}

// TODO: Hook this up to `crate::stdout`.
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
