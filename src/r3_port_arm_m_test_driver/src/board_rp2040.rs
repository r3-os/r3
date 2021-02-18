use core::panic::PanicInfo;
use r3::kernel::{cfg::CfgBuilder, Kernel, StartupHook};
use r3_support_rp2040::usbstdio;

/// The separators for our multiplexing protocol
pub mod mux {
    pub const BEGIN_MAIN: &str = "\x171";
    pub const BEGIN_LOG: &str = "\x172";
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Disable IRQ
    cortex_m::interrupt::disable();

    r3_support_rp2040::sprintln!("{}{}", mux::BEGIN_MAIN, info);

    // TODO: keep polling
    loop {}
}

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        r3_support_rp2040::sprintln!(
            "{}[{:5} {}] {}",
            mux::BEGIN_LOG,
            record.level(),
            record.target(),
            record.args()
        );
    }

    fn flush(&self) {}
}

pub const fn configure<System: Kernel>(b: &mut CfgBuilder<System>) {
    StartupHook::build()
        .start(|_| {
            // Set the correct vector table address
            unsafe {
                let p = cortex_m::Peripherals::steal();
                p.SCB.vtor.write(0x20000000);
            }

            // Configure peripherals
            let p = unsafe { rp2040::Peripherals::steal() };
            r3_support_rp2040::clock::init_clock(
                &p.CLOCKS,
                &p.XOSC,
                &p.PLL_SYS,
                &p.PLL_USB,
                &p.RESETS,
                &p.WATCHDOG,
            );

            // Reset and enable IO bank 0
            p.RESETS
                .reset
                .modify(|_, w| w.pads_bank0().set_bit().io_bank0().set_bit());
            p.RESETS
                .reset
                .modify(|_, w| w.pads_bank0().clear_bit().io_bank0().clear_bit());
            while p.RESETS.reset_done.read().pads_bank0().bit_is_clear() {}
            while p.RESETS.reset_done.read().io_bank0().bit_is_clear() {}

            // Note: CM0 don't support CAS atomics. This is why we need to use
            //       `set_logger_racy` here.
            // Safety: There are no other threads calling `set_logger_racy` at the
            //         same time.
            unsafe { log::set_logger_racy(&Logger).unwrap() };
            log::set_max_level(log::LevelFilter::Trace);
        })
        .finish(b);

    usbstdio::configure::<_, Options>(b);
}

struct Options;

impl usbstdio::Options for Options {
    /// Handle USB serial input data.
    fn handle_input(s: &[u8]) {
        for &b in s.iter() {
            match b {
                b'r' => {
                    // Restart RP2040 in BOOTSEL mode
                    let gpio_activity_pin_mask = 1 << 25; // Use GP25 as an "activity light"
                    let disable_interface_mask = 1; // enable only PICOBOOT (disable USB MSD)
                    BootromHdr::global()
                        .reset_to_usb_boot(gpio_activity_pin_mask, disable_interface_mask);
                }
                b'g' => {
                    // TODO: unblock the output? check if this is really necessary.
                    //       maybe we can get away with DTR
                }
                _ => {}
            }
        }
    }

    fn product_name() -> &'static str {
        "R3 Test Driver Port"
    }
}

#[repr(C)]
struct BootromHdr {
    // The first field is excluded because we don't want upset LLVM by a
    // null pointer
    // _initial_boot_stack_ptr: usize,
    _reset_handler: unsafe extern "C" fn(),
    _nmi_handler: unsafe extern "C" fn(),
    _hard_fault_handler: unsafe extern "C" fn(),
    _magic: [u8; 3],
    _version: u8,
    rom_func_table: BootromHalfPtr<BootromFnTablePtr>,
    rom_data_table: BootromHalfPtr<usize>,
    rom_table_lookup: BootromHalfPtr<extern "C" fn(BootromFnTablePtr, u32) -> *const ()>,
}

impl BootromHdr {
    fn global() -> &'static Self {
        unsafe { &*(4 as *const BootromHdr) }
    }

    unsafe fn lookup_func<T>(&self, c1: u8, c2: u8) -> Option<T> {
        let value = self.rom_table_lookup.get()(
            self.rom_func_table.get(),
            u32::from_le_bytes([c1, c2, 0, 0]),
        );
        unsafe { core::mem::transmute_copy(&value) }
    }

    fn reset_to_usb_boot(&self, gpio_activity_pin_mask: u32, disable_interface_mask: u32) -> ! {
        unsafe {
            self.lookup_func::<extern "C" fn(u32, u32) -> !>(b'U', b'B')
                .expect("could not locate `reset_to_usb_boot`")(
                gpio_activity_pin_mask,
                disable_interface_mask,
            )
        }
    }
}

#[repr(transparent)]
struct BootromHalfPtr<T>(u16, core::marker::PhantomData<T>);

impl<T> BootromHalfPtr<T> {
    fn get(&self) -> T {
        unsafe { core::mem::transmute_copy(&(self.0 as usize)) }
    }
}

#[repr(transparent)]
struct BootromFnTablePtr(usize);
