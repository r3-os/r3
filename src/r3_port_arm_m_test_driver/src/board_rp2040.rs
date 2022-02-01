use core::{
    panic::PanicInfo,
    sync::atomic::{AtomicBool, Ordering},
};
use r3::kernel::{traits, Cfg, StartupHook};
use r3_support_rp2040::usbstdio;

/// The separators for our multiplexing protocol
pub mod mux {
    pub const BEGIN_MAIN: &str = "\x171";
    pub const BEGIN_LOG: &str = "\x172";
}

pub const SYSTICK_FREQUENCY: u64 = 48_000_000;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Disable IRQ
    cortex_m::interrupt::disable();

    r3_support_rp2040::sprintln!("{}{}", mux::BEGIN_MAIN, info);

    enter_poll_loop();
}

/// Start polling USB so that we can deliver the test result and reset the
/// device when requested.
pub fn enter_poll_loop() -> ! {
    loop {
        usbstdio::poll::<Options>();
    }
}

/// Implements `kernel_benchmarks::Driver::performance_time`.
pub fn performance_time() -> u32 {
    let timerawl = unsafe { (0x40054028 as *const u32).read_volatile() };
    timerawl.wrapping_mul(2) // scale by `tick.cycles`
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

pub const fn configure<C>(b: &mut Cfg<C>)
where
    C: ~const traits::CfgBase + ~const traits::CfgInterruptLine,
    C::System: traits::KernelInterruptLine,
{
    StartupHook::define()
        .start(|| {
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

            // clk_ref → clk_sys = 48MHz
            p.CLOCKS.clk_sys_ctrl.modify(|_, w| w.src().clk_ref());

            // Supply clk_ref / 2 = 24MHz to SysTick, watchdog, and timer
            // because we want to measure times at high precision in
            // benchmarks. Setting `cycles = 1` would be ideal but doesn't work
            // for some reason.
            p.WATCHDOG
                .tick
                .write(|b| unsafe { b.cycles().bits(2).enable().set_bit() });

            // Reset the timer used by `performance_time`
            p.RESETS.reset.modify(|_, w| w.timer().set_bit());
            p.RESETS.reset.modify(|_, w| w.timer().clear_bit());
            while p.RESETS.reset_done.read().timer().bit_is_clear() {}

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

static SHOULD_PAUSE_OUTPUT: AtomicBool = AtomicBool::new(true);

struct Options;

impl usbstdio::Options for Options {
    /// Handle USB serial input data.
    fn handle_input(s: &[u8]) {
        let mut should_unpause_output = false;
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
                    should_unpause_output = true;
                }
                _ => {}
            }
        }

        if should_unpause_output && SHOULD_PAUSE_OUTPUT.load(Ordering::Relaxed) {
            SHOULD_PAUSE_OUTPUT.store(false, Ordering::Relaxed);

            // Flush the transmission buffer.
            usbstdio::poll::<Options>();
        }
    }

    fn product_name() -> &'static str {
        "R3 Test Driver Port"
    }

    fn should_pause_output() -> bool {
        SHOULD_PAUSE_OUTPUT.load(Ordering::Relaxed)
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
