// This module is only intended to be used internally, hence the semver
// exemption. It probably should be in a HAL or board crate.
#![cfg(feature = "semver-exempt")]
use rp2040_pac::USBCTRL_REGS;
use usb_device::{
    endpoint::{EndpointAddress, EndpointType},
    UsbDirection,
};
use vcell::VolatileCell;

const DPRAM_LEN: usize = 0x1_0000;
const DPRAM_EP0_BUFFER_OFFSET: usize = 0x100;

const EP_CTRL_ENABLE: u32 = 1 << 31;
const EP_CTRL_INTERRUPT_PER_BUFFER: u32 = 1 << 29;
const EP_CTRL_TYPE_LSB: u32 = 26;
const EP_BUF_CTRL_AVAIL: u32 = 1 << 10;
const EP_BUF_CTRL_STALL: u32 = 1 << 11;
const EP_BUF_CTRL_PID_DATA1: u32 = 1 << 13;
const EP_BUF_CTRL_FULL: u32 = 1 << 15;
const EP_BUF_CTRL_LEN_MASK: u32 = 0x3ff;

pub struct UsbBus {
    usbctrl_regs: USBCTRL_REGS,
    ep_allocation: [u16; 2],
    ep_buffer_offset: [u16; 32],
    ep_max_packet_size: [u16; 16],
    ep_in_ready: VolatileCell<u16>,
    next_data_buffer_offset: usize,
}

unsafe impl Sync for UsbBus {}

#[inline]
fn address_to_index(addr: EndpointAddress) -> usize {
    u8::from(addr).rotate_left(1) as usize ^ 1
}

impl UsbBus {
    pub fn new(usbctrl_regs: USBCTRL_REGS) -> Self {
        Self {
            usbctrl_regs,
            ep_allocation: [0b0000_0000_0000_0001; 2],
            ep_buffer_offset: {
                let mut b = [0; 32];
                b[0] = DPRAM_EP0_BUFFER_OFFSET as _;
                b[1] = DPRAM_EP0_BUFFER_OFFSET as _;
                b
            },
            ep_max_packet_size: [0; 16],
            ep_in_ready: VolatileCell::new(0xffff),
            next_data_buffer_offset: 0x180,
        }
    }

    #[inline]
    fn usbctrl_dpram_u32(&self) -> &[VolatileCell<u32>; DPRAM_LEN / 4] {
        // We have `usbctrl_regs`, so it's probably safe to assume we have
        // access to the DPRAM, too
        unsafe { &*(0x5010_0000 as *const _) }
    }

    #[inline]
    fn usbctrl_dpram_u8(&self) -> &[VolatileCell<u8>; DPRAM_LEN] {
        // We have `usbctrl_regs`, so it's probably safe to assume we have
        // access to the DPRAM, too
        unsafe { &*(0x5010_0000 as *const _) }
    }

    #[inline]
    fn ep_ctrl(&self, addr: EndpointAddress) -> &VolatileCell<u32> {
        debug_assert!(addr.index() < 16);
        &self.usbctrl_dpram_u32()[address_to_index(addr)]
    }

    #[inline]
    fn ep_buf_ctrl(&self, addr: EndpointAddress) -> &VolatileCell<u32> {
        debug_assert!(addr.index() < 16);
        &self.usbctrl_dpram_u32()[address_to_index(addr) + 32]
    }
}

impl usb_device::bus::UsbBus for UsbBus {
    fn alloc_ep(
        &mut self,
        ep_dir: UsbDirection,
        ep_addr: Option<EndpointAddress>,
        ep_type: EndpointType,
        max_packet_size: u16,
        _interval: u8,
    ) -> usb_device::Result<EndpointAddress> {
        match ep_addr {
            Some(ep) if ep.index() == 0 => {
                self.ep_buf_ctrl(ep).set(0);

                if ep_dir == UsbDirection::Out {
                    self.ep_max_packet_size[0] = max_packet_size;
                }

                // EP0 is treated specially by the hardware
                return Ok(ep);
            }
            _ => {}
        }

        assert_eq!(UsbDirection::Out as usize >> 7, 0);
        assert_eq!(UsbDirection::In as usize >> 7, 1);
        let ep_dir_bit = (ep_dir as usize) >> 7;

        let requested_allocation_i = if let Some(ep_addr) = ep_addr {
            debug_assert_eq!(ep_dir, ep_addr.direction());
            if ep_addr.index() < 16 {
                ep_addr.index() as u32
            } else {
                // index is out of range; ignore it
                0
            }
        } else {
            0
        };

        // Find a free endpoint.
        let ep_index = (self.ep_allocation[ep_dir_bit].rotate_right(requested_allocation_i) as u32)
            .trailing_ones();
        if ep_index == 32 {
            return Err(usb_device::UsbError::EndpointOverflow);
        }
        let ep_index = (requested_allocation_i + ep_index) % 16;

        // The endpoint address
        let ep = EndpointAddress::from_parts(ep_index as usize, ep_dir);

        // Allocate the buffer
        let buffer_offset = self.next_data_buffer_offset;
        let buffer_offset_end =
            (self.next_data_buffer_offset + max_packet_size as usize + 63) & !63;
        if buffer_offset_end > DPRAM_LEN {
            return Err(usb_device::UsbError::EndpointMemoryOverflow);
        }

        // Commit the change
        self.ep_ctrl(ep).set(
            EP_CTRL_ENABLE
                | EP_CTRL_INTERRUPT_PER_BUFFER
                | ((ep_type as u32) << EP_CTRL_TYPE_LSB)
                | (buffer_offset as u32),
        );

        if ep_dir == UsbDirection::Out {
            // Get ready to receive
            self.ep_buf_ctrl(ep)
                .set(EP_BUF_CTRL_AVAIL | max_packet_size as u32);
            self.ep_max_packet_size[ep.index()] = max_packet_size;
        } else {
            self.ep_buf_ctrl(ep).set(EP_BUF_CTRL_PID_DATA1);
        }

        self.ep_buffer_offset[address_to_index(ep)] = buffer_offset as _;

        self.next_data_buffer_offset = buffer_offset_end;
        self.ep_allocation[ep_dir_bit] |= 1 << ep_index;

        Ok(ep)
    }

    fn enable(&mut self) {
        // Mux the controller to the onboard usb phy
        self.usbctrl_regs
            .usb_muxing
            .write(|bits| bits.to_phy().set_bit().softcon().set_bit());

        // Force VBUS detect so the device thinks it is plugged into a host
        self.usbctrl_regs.usb_pwr.write(|bits| {
            bits.vbus_detect()
                .set_bit()
                .vbus_detect_override_en()
                .set_bit()
        });

        // Choose Device mode, enable the controller
        self.usbctrl_regs
            .main_ctrl
            .write(|bits| bits.host_ndevice().clear_bit().controller_en().set_bit());

        // Set bit in BUFF_STATUS for every buffer completed RW 0x0 on EP0
        self.usbctrl_regs
            .sie_ctrl
            .write(|bits| bits.ep0_int_1buf().set_bit());

        // Interrupt enable
        self.usbctrl_regs.inte.write(|bits| {
            bits.buff_status()
                .set_bit()
                .bus_reset()
                .set_bit()
                .setup_req()
                .set_bit()
        });

        // Indicate device connection by enabling pull-up resistors on D+/D-
        // TODO: use bus RMW ops
        self.usbctrl_regs
            .sie_ctrl
            .modify(|_, w| w.pullup_en().set_bit());
    }

    fn reset(&self) {
        self.ep_buf_ctrl(EndpointAddress::from_parts(0, UsbDirection::In))
            .set(0);

        // TODO: putting the correct value causes a protocol error. Why?
        self.ep_buf_ctrl(EndpointAddress::from_parts(0, UsbDirection::Out))
            .set(0);

        self.ep_in_ready.set(0xffff);

        for i in 1..16 {
            if (self.ep_allocation[0] & (1 << i)) != 0 {
                let ep = EndpointAddress::from_parts(i, UsbDirection::Out);
                let max_packet_size = self.ep_max_packet_size[i];
                self.ep_buf_ctrl(ep)
                    .set(EP_BUF_CTRL_AVAIL | max_packet_size as u32);
            }

            if (self.ep_allocation[1] & (1 << i)) != 0 {
                let ep = EndpointAddress::from_parts(i, UsbDirection::In);
                self.ep_buf_ctrl(ep).set(EP_BUF_CTRL_PID_DATA1);
            }
        }
    }

    #[inline]
    fn set_device_address(&self, addr: u8) {
        self.usbctrl_regs
            .addr_endp
            .write(|b| unsafe { b.address().bits(addr) });
    }

    fn write(&self, ep_addr: EndpointAddress, buf: &[u8]) -> usb_device::Result<usize> {
        debug_assert!(ep_addr.direction() == UsbDirection::In);

        let ep_i = address_to_index(ep_addr);
        let buf_ctrl = self.ep_buf_ctrl(ep_addr);

        if (self.ep_in_ready.get() & (1 << ep_addr.index())) == 0 {
            return Err(usb_device::UsbError::WouldBlock);
        }

        if ep_i == 0 {
            // When writing to EP0 IN, reset EP0 OUT's state. We could be too
            // late if we tried to do this when a SETUP packet is received.
            self.ep_buf_ctrl(EndpointAddress::from_parts(0, UsbDirection::Out))
                .set(
                    EP_BUF_CTRL_PID_DATA1 | EP_BUF_CTRL_AVAIL | (self.ep_max_packet_size[0] as u32),
                );
        }

        let hw_buf = &self.usbctrl_dpram_u8()[self.ep_buffer_offset[ep_i] as _..];
        for (x, y) in hw_buf.iter().zip(buf.iter()) {
            x.set(*y);
        }

        // Set `buf_ctrl`, toggle PID
        buf_ctrl.set(
            buf_ctrl.get() & !EP_BUF_CTRL_AVAIL & !EP_BUF_CTRL_LEN_MASK ^ EP_BUF_CTRL_PID_DATA1
                | buf.len() as u32
                | EP_BUF_CTRL_FULL,
        );

        // 12 cycle delay
        #[cfg(target_arch = "arm")]
        unsafe {
            core::arch::asm!(
                "b 1f
                    1: b 1f
                    1: b 1f
                    1: b 1f
                    1: b 1f
                    1: b 1f
                    1:\n"
            );
        }

        buf_ctrl.set(buf_ctrl.get() | EP_BUF_CTRL_AVAIL);

        self.ep_in_ready
            .set(self.ep_in_ready.get() & !(1u16 << ep_addr.index()));

        Ok(buf.len())
    }

    fn read(&self, ep_addr: EndpointAddress, buf: &mut [u8]) -> usb_device::Result<usize> {
        debug_assert!(ep_addr.direction() == UsbDirection::Out);

        let ep_i = address_to_index(ep_addr);
        if ep_i == 1 && self.usbctrl_regs.ints.read().setup_req().bit() {
            // The setup packet for EP0 gets written to the first eight bytes
            // of the DPRAM
            let words = [
                self.usbctrl_dpram_u32()[0].get(),
                self.usbctrl_dpram_u32()[1].get(),
            ];

            let buf = buf
                .get_mut(..8)
                .ok_or(usb_device::UsbError::BufferOverflow)?;
            buf[0..4].copy_from_slice(&words[0].to_ne_bytes());
            buf[4..8].copy_from_slice(&words[1].to_ne_bytes());

            // Clear the setup request status
            self.usbctrl_regs
                .sie_status
                .write(|b| b.setup_rec().set_bit());

            return Ok(8);
        }

        if self.usbctrl_regs.buff_status.read().bits() & (1 << ep_i) == 0 {
            return Err(usb_device::UsbError::WouldBlock);
        }

        let buf_ctrl = self.ep_buf_ctrl(ep_addr);

        // Copy from the hardware buffer
        let len = (buf_ctrl.get() & EP_BUF_CTRL_LEN_MASK) as usize;
        let buf = buf
            .get_mut(..len)
            .ok_or(usb_device::UsbError::BufferOverflow)?;

        // TODO: Optimize buffer copy
        let hw_buf = &self.usbctrl_dpram_u8()[self.ep_buffer_offset[ep_i] as _..];
        for (x, y) in hw_buf.iter().zip(buf.iter_mut()) {
            *y = x.get();
        }

        // Flip the expected PID, mark the buffer as empty
        buf_ctrl.set(
            buf_ctrl.get() & !EP_BUF_CTRL_FULL & !EP_BUF_CTRL_LEN_MASK ^ EP_BUF_CTRL_PID_DATA1
                | EP_BUF_CTRL_AVAIL
                | (self.ep_max_packet_size[ep_addr.index()] as u32),
        );

        // Clear the status bit
        // FIXME: `buff_status` is RO in SVD and the RP2040 manual, but the
        // example code does write it
        unsafe {
            (&raw const self.usbctrl_regs.buff_status as *mut u32).write_volatile(1 << ep_i);
        }

        Ok(buf.len())
    }

    #[inline]
    fn set_stalled(&self, ep_addr: EndpointAddress, stalled: bool) {
        let buf_ctrl = self.ep_buf_ctrl(ep_addr);
        if stalled {
            buf_ctrl.set(buf_ctrl.get() | EP_BUF_CTRL_STALL);
        } else {
            buf_ctrl.set(buf_ctrl.get() & !EP_BUF_CTRL_STALL);
        }

        if ep_addr.index() == 0 {
            self.usbctrl_regs
                .ep_stall_arm
                .modify(|_, w| match (ep_addr.direction(), stalled) {
                    (UsbDirection::In, false) => w.ep0_in().clear_bit(),
                    (UsbDirection::In, true) => w.ep0_in().set_bit(),
                    (UsbDirection::Out, false) => w.ep0_out().clear_bit(),
                    (UsbDirection::Out, true) => w.ep0_out().set_bit(),
                });
        }
    }

    #[inline]
    fn is_stalled(&self, ep_addr: EndpointAddress) -> bool {
        (self.ep_buf_ctrl(ep_addr).get() & EP_BUF_CTRL_STALL) != 0
    }

    fn suspend(&self) {}

    fn resume(&self) {
        // TODO: Remote resume
    }

    #[inline]
    fn poll(&self) -> usb_device::bus::PollResult {
        let status = self.usbctrl_regs.ints.read();

        if status.bus_reset().bit() {
            // Clear the device address
            self.usbctrl_regs
                .addr_endp
                .write(|b| unsafe { b.address().bits(0) });

            // Clear the bus reset status
            self.usbctrl_regs
                .sie_status
                .write(|b| b.bus_reset().set_bit());

            return usb_device::bus::PollResult::Reset;
        }

        let mut ep_out = 0u16;
        let mut ep_in_complete = 0u16;
        let mut ep_setup = 0u16;

        if status.setup_req().bit() {
            ep_setup |= 1;

            // The first DATA packet to receive by EP0 OUT must be DATA1.
            // However, we can't modify `ep_buf_ctrl((0, Out))` at this point
            // because it may already have a valid PID (DATA0/DATA1) and the
            // hardware may already be receiving the first packet. Therefore,
            // we do this when writing to EP0 In.

            // The first DATA packet to send to EP0 IN must be DATA1
            self.ep_buf_ctrl(EndpointAddress::from_parts(0, UsbDirection::In))
                .set(0 /* clear `EP_BUF_CTRL_PID_DATA1` */);
        }

        if status.buff_status().bit() {
            let buf_status = self.usbctrl_regs.buff_status.read().bits();

            // IN EP (i / 2), i = 0, 2, 4, ..., 30
            {
                let mut buf_status = buf_status;
                let mut b2 = 1; // 1 << (i / 2)
                loop {
                    if buf_status == 0 {
                        break;
                    }

                    if (buf_status & 1) != 0 {
                        // ep_in_complete |= 1 << (i / 2)
                        ep_in_complete |= b2;
                    }

                    // i += 2;
                    buf_status >>= 2;
                    b2 <<= 1;
                }
                self.ep_in_ready
                    .set(self.ep_in_ready.get() | ep_in_complete);
            }

            // OUT EP ((i - 1) / 2), i = 1, 3, 5, ..., 31
            {
                let mut buf_status = buf_status >> 1;
                let mut b2 = 1; // 1 << ((i - 1) / 2)
                loop {
                    if buf_status == 0 {
                        break;
                    }

                    if (buf_status & 1) != 0 {
                        // ep_out |= 1 << ((i - 1) / 2)
                        ep_out |= b2;
                    }

                    // i += 2;
                    buf_status >>= 2;
                    b2 <<= 1;
                }
            }

            // Clear the status bits for IN endpoints
            //
            // FIXME: `buff_status` is RO in SVD and the RP2040 manual, but the
            // example code does write it:
            // <https://github.com/raspberrypi/tinyusb/blob/e0aa405d19e35dbf58cf502b8106455c1a3c2a5c/src/portable/raspberrypi/rp2040/dcd_rp2040.c#L225>
            let buf_status_cleared_bits = buf_status & 0x55555555;
            unsafe {
                (&raw const self.usbctrl_regs.buff_status as *mut u32)
                    .write_volatile(buf_status_cleared_bits);
            }
        }

        // It's harmless to return `Data` when there are no events in the
        // current implementatino of `usb_device`.
        usb_device::bus::PollResult::Data {
            ep_out,
            ep_in_complete,
            ep_setup,
        }
    }

    const QUIRK_SET_ADDRESS_BEFORE_STATUS: bool = false;

    fn force_reset(&self) -> usb_device::Result<()> {
        // Indicate device disconnection by disabling pull-up resistors on D+/D-
        // TODO: use bus RMW ops
        self.usbctrl_regs
            .sie_ctrl
            .modify(|_, w| w.pullup_en().clear_bit());

        // And re-enable them
        self.usbctrl_regs
            .sie_ctrl
            .modify(|_, w| w.pullup_en().set_bit());

        Ok(())
    }
}
