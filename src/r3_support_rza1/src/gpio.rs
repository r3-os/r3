//! Pin configuration
// This module is only intended to be used internally, hence the semver
// exemption. It probably should be in a HAL crate, but there's no HAL crate
// for RZ/A1.
#![cfg(feature = "semver-exempt")]

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum AltMode {
    Gpio,
    Alt1,
    Alt2,
    Alt3,
    Alt4,
    Alt5,
    Alt6,
    Alt7,
    Alt8,
}

pub type Pin = (u8, u8);

pub trait GpioExt {
    /// Configure the alternate function mode of the specified pin.
    fn set_alt_mode(&self, pin: Pin, mode: AltMode) -> &Self;

    /// Configure the specified pin for output.
    fn set_output(&self, pin: Pin) -> &Self;

    /// Configure the specified pin for input.
    fn set_input(&self, pin: Pin) -> &Self;
}

unsafe fn set_bit16(reg: *mut u16, bit: u8) {
    unsafe { reg.write_volatile(reg.read_volatile() | (1u16 << bit)) };
}

unsafe fn clear_bit16(reg: *mut u16, bit: u8) {
    unsafe { reg.write_volatile(reg.read_volatile() & !(1u16 << bit)) };
}

#[inline]
fn panic_if_pin_is_invalid((n, m): Pin) {
    assert!(n >= 1 && n < 12, "1 <= {} < 12", n);
    assert!(m < 16, "0 <= {} < 16", m);
}

impl GpioExt for rza1::gpio::RegisterBlock {
    #[inline]
    fn set_alt_mode(&self, (n, m): Pin, mode: AltMode) -> &Self {
        panic_if_pin_is_invalid((n, m));

        let pmc = (&self.pmc1 as *const _ as *mut u16).wrapping_add((n - 1) as usize * 2);
        let pfcae = (&self.pfcae1 as *const _ as *mut u16).wrapping_add((n - 1) as usize * 2);
        let pfce = (&self.pfce1 as *const _ as *mut u16).wrapping_add((n - 1) as usize * 2);
        let pfc = (&self.pfc1 as *const _ as *mut u16).wrapping_add((n - 1) as usize * 2);

        unsafe {
            match mode {
                AltMode::Gpio => {
                    clear_bit16(pmc, m);
                }
                AltMode::Alt1 => {
                    set_bit16(pmc, m);
                    clear_bit16(pfcae, m);
                    clear_bit16(pfce, m);
                    clear_bit16(pfc, m);
                }
                AltMode::Alt2 => {
                    set_bit16(pmc, m);
                    clear_bit16(pfcae, m);
                    clear_bit16(pfce, m);
                    set_bit16(pfc, m);
                }
                AltMode::Alt3 => {
                    set_bit16(pmc, m);
                    clear_bit16(pfcae, m);
                    set_bit16(pfce, m);
                    clear_bit16(pfc, m);
                }
                AltMode::Alt4 => {
                    set_bit16(pmc, m);
                    clear_bit16(pfcae, m);
                    set_bit16(pfce, m);
                    set_bit16(pfc, m);
                }
                AltMode::Alt5 => {
                    set_bit16(pmc, m);
                    set_bit16(pfcae, m);
                    clear_bit16(pfce, m);
                    clear_bit16(pfc, m);
                }
                AltMode::Alt6 => {
                    set_bit16(pmc, m);
                    set_bit16(pfcae, m);
                    clear_bit16(pfce, m);
                    set_bit16(pfc, m);
                }
                AltMode::Alt7 => {
                    set_bit16(pmc, m);
                    set_bit16(pfcae, m);
                    set_bit16(pfce, m);
                    clear_bit16(pfc, m);
                }
                AltMode::Alt8 => {
                    set_bit16(pmc, m);
                    set_bit16(pfcae, m);
                    set_bit16(pfce, m);
                    set_bit16(pfc, m);
                }
            }
        }
        self
    }

    #[inline]
    fn set_output(&self, (n, m): Pin) -> &Self {
        panic_if_pin_is_invalid((n, m));
        let pm = (&self.pm1 as *const _ as *mut u16).wrapping_add((n - 1) as usize * 2);
        unsafe { clear_bit16(pm, m) };
        self
    }

    #[inline]
    fn set_input(&self, (n, m): Pin) -> &Self {
        panic_if_pin_is_invalid((n, m));
        let pm = (&self.pm1 as *const _ as *mut u16).wrapping_add((n - 1) as usize * 2);
        unsafe { set_bit16(pm, m) };
        self
    }
}
