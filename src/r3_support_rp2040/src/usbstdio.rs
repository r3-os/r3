//! Standard input/output, behaving as a USB serial device
// This module is only intended to be used internally, hence the semver
// exemption. It probably should be in a HAL or board crate.
use crate::usb::UsbBus;
use core::mem::MaybeUninit;
use r3::kernel::{
    cfg::CfgBuilder, InterruptHandler, InterruptLine, InterruptNum, Kernel, StartupHook,
};
use usb_device::{
    bus::UsbBusAllocator,
    device::{UsbDevice, UsbDeviceBuilder, UsbVidPid},
};
use usbd_serial::{SerialPort, USB_CLASS_CDC};

// TODO: Get rid of these awful `static mut`s
static mut USB_BUS_ALLOCATOR: MaybeUninit<UsbBusAllocator<UsbBus>> = MaybeUninit::uninit();
static mut USB_DEVICE: MaybeUninit<UsbDevice<'_, UsbBus>> = MaybeUninit::uninit();
static mut SERIAL: MaybeUninit<SerialPort<'static, UsbBus>> = MaybeUninit::uninit();

pub const fn configure<System: Kernel>(b: &mut CfgBuilder<System>) {
    StartupHook::build()
        .start(|_| unsafe {
            let p = rp2040::Peripherals::steal();

            // Reset PLL
            p.RESETS.reset.modify(|_, w| w.usbctrl().set_bit());
            p.RESETS.reset.modify(|_, w| w.usbctrl().clear_bit());
            while p.RESETS.reset_done.read().usbctrl().bit_is_clear() {}

            USB_BUS_ALLOCATOR = MaybeUninit::new(UsbBusAllocator::new(UsbBus::new(p.USBCTRL_REGS)));

            SERIAL = MaybeUninit::new(SerialPort::new(USB_BUS_ALLOCATOR.assume_init_ref()));

            USB_DEVICE = MaybeUninit::new(
                UsbDeviceBuilder::new(
                    USB_BUS_ALLOCATOR.assume_init_ref(),
                    UsbVidPid(0x16c0, 0x27dd),
                )
                .product("r3_support_rp2040 standard I/O")
                .device_class(USB_CLASS_CDC)
                .max_packet_size_0(64)
                .build(),
            );
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
            let usb_device = unsafe { USB_DEVICE.assume_init_mut() };
            let serial = unsafe { SERIAL.assume_init_mut() };
            usb_device.poll(&mut [serial]);

            let mut buf = [0; 64];
            if let Ok(len) = serial.read(&mut buf) {
                if len > 0 {
                    // TEST
                    crate::sprintln!("{:?}", &buf[..len]);
                    if buf[..len] == b"\r"[..] || buf[..len] == b"\n"[..] {
                        let _ = serial.write(b"\r\n");
                        return;
                    }
                    let _ = serial.write(&[b'[']);
                    let _ = serial.write(&buf[..len]);
                    let _ = serial.write(&[b']']);
                }
            }
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
