#![feature(asm)]
#![feature(const_fn_trait_bound)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_mut_refs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unsupported_naked_functions)]
#![no_std]
#![no_main]
#![cfg(target_os = "none")]
use r3::{
    kernel::{cfg::CfgBuilder, StartupHook, Task},
    prelude::*,
    sync::Mutex,
};
use r3_port_arm_m as port;
use r3_support_rp2040 as support_rp2040;

// --------------------------------------------------------------------------
// Target-specific configuration

// The second-level bootloader, which is responsible for configuring execute-in-
// place. The bootrom copies this into SRAM and executes it.
#[link_section = ".boot_loader"]
#[used]
pub static BOOT_LOADER: [u8; 256] = rp2040_boot2::BOOT_LOADER;

mod panic_serial;

port::use_port!(unsafe struct System);
port::use_rt!(unsafe System);
port::use_systick_tickful!(unsafe impl PortTimer for System);

impl port::ThreadingOptions for System {}

impl port::SysTickOptions for System {
    // "The timer uses a one microsecond reference that is generated in the
    // Watchdog (see Section 4.7.2) which comes from clk_ref."
    const FREQUENCY: u64 = 1_000_000;
}

const USE_USB_UART: bool = true;

impl support_rp2040::usbstdio::Options for System {
    fn handle_input(s: &[u8]) {
        if s == b"\r" || s == b"\n" {
            support_rp2040::sprint!("\n");
            return;
        }

        // echo the input with brackets
        if let Ok(s) = core::str::from_utf8(s) {
            support_rp2040::sprint!("[{}]", s);
        } else {
            support_rp2040::sprint!("[<not UTF-8>]");
        }
    }
}

// --------------------------------------------------------------------------

#[derive(Debug)]
struct Objects {
    task1: Task<System>,
    task2: Task<System>,
    mutex1: Mutex<System, u32>,
}

const COTTAGE: Objects = r3::build!(System, configure_app => Objects);

const fn configure_app(b: &mut CfgBuilder<System>) -> Objects {
    b.num_task_priority_levels(4);

    StartupHook::build()
        .start(|_| {
            // Configure peripherals
            let p = unsafe { rp2040::Peripherals::steal() };
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
        })
        .finish(b);

    if USE_USB_UART {
        support_rp2040::usbstdio::configure::<System, System>(b);
    }

    System::configure_systick(b);

    let task1 = Task::build()
        .start(task1_body)
        .priority(2)
        .active(true)
        .finish(b);
    let task2 = Task::build().start(task2_body).priority(3).finish(b);

    let mutex1 = Mutex::build().finish(b);

    Objects {
        task1,
        task2,
        mutex1,
    }
}

fn task1_body(_: usize) {
    support_rp2040::sprintln!("COTTAGE = {:?}", COTTAGE);

    COTTAGE.task2.activate().unwrap();
}

fn task2_body(_: usize) {
    let p = unsafe { rp2040::Peripherals::steal() };

    // <https://github.com/jannic/rp-microcontroller-rs/blob/master/boards/rp-pico/examples/blink/main.rs>
    // TODO: Documentate what this code does
    let pin = 25;

    p.SIO.gpio_oe_clr.write(|w| unsafe { w.bits(1 << pin) });
    p.SIO.gpio_out_clr.write(|w| unsafe { w.bits(1 << pin) });

    p.PADS_BANK0
        .gpio25
        .write(|w| w.ie().bit(true).od().bit(false));

    p.IO_BANK0.gpio25_ctrl.write(|w| w.funcsel().sio_25());

    p.SIO.gpio_oe_set.write(|w| unsafe { w.bits(1 << pin) });
    p.SIO.gpio_out_set.write(|w| unsafe { w.bits(1 << pin) });

    loop {
        // Blink the LED
        p.SIO.gpio_out_set.write(|w| unsafe { w.bits(1 << pin) });
        System::sleep(r3::time::Duration::from_millis(100)).unwrap();
        p.SIO.gpio_out_clr.write(|w| unsafe { w.bits(1 << pin) });

        support_rp2040::sprintln!("time = {:?}", System::time().unwrap());
        System::sleep(r3::time::Duration::from_millis(900)).unwrap();
    }
}
