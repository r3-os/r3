#![feature(const_refs_to_cell)]
#![feature(const_trait_impl)]
#![feature(naked_functions)]
#![feature(const_mut_refs)]
#![feature(asm_const)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]
#![cfg(target_os = "none")]
use r3::{
    bind::bind,
    kernel::{prelude::*, StaticTask},
    prelude::*,
    sync::StaticMutex,
};
use r3_port_arm_m as port;
use r3_support_rp2040 as support_rp2040;

// --------------------------------------------------------------------------
// Target-specific configuration

// The second-level bootloader, which is responsible for configuring execute-in-
// place. The bootrom copies this into SRAM and executes it.
#[link_section = ".boot_loader"]
#[used]
pub static BOOT_LOADER: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

mod panic_serial;

type System = r3_kernel::System<SystemTraits>;
port::use_port!(unsafe struct SystemTraits);
port::use_rt!(unsafe SystemTraits);
port::use_systick_tickful!(unsafe impl PortTimer for SystemTraits);

impl port::ThreadingOptions for SystemTraits {}

impl port::SysTickOptions for SystemTraits {
    // "The timer uses a one microsecond reference that is generated in the
    // Watchdog (see Section 4.7.2) which comes from clk_ref."
    const FREQUENCY: u64 = 1_000_000;
}

const USE_USB_UART: bool = true;

impl support_rp2040::usbstdio::Options for SystemTraits {
    fn handle_input(s: &[u8]) {
        if s == b"\r" || s == b"\n" {
            support_rp2040::sprint!("\n");
            return;
        }

        // echo the input with brackets
        if let Ok(s) = core::str::from_utf8(s) {
            support_rp2040::sprint!("[{s}]");
        } else {
            support_rp2040::sprint!("[<not UTF-8>]");
        }
    }
}

// --------------------------------------------------------------------------

#[derive(Debug)]
struct Objects {
    #[allow(dead_code)]
    task1: StaticTask<System>,
    task2: StaticTask<System>,
    #[allow(dead_code)]
    mutex1: StaticMutex<System, u32>,
}

const COTTAGE: Objects = r3_kernel::build!(SystemTraits, configure_app => Objects);

const fn configure_app(b: &mut r3_kernel::Cfg<SystemTraits>) -> Objects {
    b.num_task_priority_levels(4);

    let (rp2040_resets, rp2040_usbctrl_regs, rp2040_sio, rp2040_pads_bank0, rp2040_io_bank0) =
        bind((), || {
            // Configure peripherals
            let p = rp2040_pac::Peripherals::take().unwrap();
            support_rp2040::clock::init_clock(
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

            if !USE_USB_UART {
                // Confiugre UART0
                use support_rp2040::serial::UartExt;
                let uart0 = p.UART0;
                uart0.reset(&p.RESETS);
                uart0.configure_pins(&p.IO_BANK0);
                uart0.configure_uart(115_200);

                support_rp2040::stdout::set_stdout(uart0.into_nb_writer());
            }

            (p.RESETS, p.USBCTRL_REGS, p.SIO, p.PADS_BANK0, p.IO_BANK0)
        })
        .unpure()
        .finish(b)
        .unzip();

    if USE_USB_UART {
        support_rp2040::usbstdio::configure::<_, SystemTraits>(
            b,
            rp2040_resets,
            rp2040_usbctrl_regs,
        );
    }

    SystemTraits::configure_systick(b);

    let task1 = StaticTask::define()
        .start(task1_body)
        .priority(2)
        .active(true)
        .finish(b);
    let task2 = StaticTask::define()
        .start_with_bind(
            (
                rp2040_sio.borrow_mut(),
                rp2040_pads_bank0.borrow_mut(),
                rp2040_io_bank0.borrow_mut(),
            ),
            task2_body,
        )
        .priority(3)
        .finish(b);

    let mutex1 = StaticMutex::define().finish(b);

    Objects {
        task1,
        task2,
        mutex1,
    }
}

fn task1_body() {
    support_rp2040::sprintln!("COTTAGE = {COTTAGE:?}");

    COTTAGE.task2.activate().unwrap();
}

fn task2_body(
    rp2040_sio: &mut rp2040_pac::SIO,
    rp2040_pads_bank0: &mut rp2040_pac::PADS_BANK0,
    rp2040_io_bank0: &mut rp2040_pac::IO_BANK0,
) {
    // <https://github.com/jannic/rp-microcontroller-rs/blob/master/boards/rp-pico/examples/blink/main.rs>
    // TODO: Documentate what this code does
    let pin = 25;

    rp2040_sio
        .gpio_oe_clr
        .write(|w| unsafe { w.bits(1 << pin) });
    rp2040_sio
        .gpio_out_clr
        .write(|w| unsafe { w.bits(1 << pin) });

    rp2040_pads_bank0.gpio[25].write(|w| w.ie().bit(true).od().bit(false));

    rp2040_io_bank0.gpio[25]
        .gpio_ctrl
        .write(|w| w.funcsel().sio());

    rp2040_sio
        .gpio_oe_set
        .write(|w| unsafe { w.bits(1 << pin) });
    rp2040_sio
        .gpio_out_set
        .write(|w| unsafe { w.bits(1 << pin) });

    loop {
        // Blink the LED
        rp2040_sio
            .gpio_out_set
            .write(|w| unsafe { w.bits(1 << pin) });
        System::sleep(r3::time::Duration::from_millis(100)).unwrap();
        rp2040_sio
            .gpio_out_clr
            .write(|w| unsafe { w.bits(1 << pin) });

        support_rp2040::sprintln!("time = {:?}", System::time().unwrap());
        System::sleep(r3::time::Duration::from_millis(900)).unwrap();
    }
}
