#![feature(asm)]
#![feature(const_fn)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_mut_refs)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]
#![cfg(target_os = "none")]
use r3::{
    kernel::{cfg::CfgBuilder, Task},
    prelude::*,
    sync::Mutex,
};
use r3_port_arm_m as port;

// --------------------------------------------------------------------------
// Target-specific configuration

// The second-level bootloader, which is responsible for configuring execute-in-
// place. The bootrom copies this into SRAM and executes it.
#[link_section = ".boot_loader"]
#[used]
pub static BOOT_LOADER: [u8; 256] = rp2040_boot2::BOOT_LOADER;

mod panic_serial;

port::use_port!(unsafe struct System);
port::use_systick_tickful!(unsafe impl PortTimer for System);

impl port::ThreadingOptions for System {}

impl port::SysTickOptions for System {
    // "The timer uses a one microsecond reference that is generated in the
    // Watchdog (see Section 4.7.2) which comes from clk_ref." It's unclear
    // whether this applies to SysTick with CLKSOURCE = 0.
    const FREQUENCY: u64 = 1_000_000;
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

    // TODO: Configure XOSC
    // TODO: Configure USB serial

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
    // TODO: rtt_target::rprintln!("COTTAGE = {:?}", COTTAGE);

    COTTAGE.task2.activate().unwrap();
}

fn task2_body(_: usize) {
    let p = unsafe { rp2040::Peripherals::steal() };

    // <https://github.com/jannic/rp-microcontroller-rs/blob/master/boards/rp-pico/examples/blink/main.rs>
    // TODO: Documentate what this code does
    let pin = 25;
    p.RESETS.reset.modify(|r, w| {
        unsafe { w.bits(r.bits()) }
            .pads_bank0()
            .clear_bit()
            .io_bank0()
            .clear_bit()
    });

    loop {
        let r = p.RESETS.reset_done.read();
        if r.pads_bank0().bit() && r.io_bank0().bit() {
            break;
        }
    }

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

        // TODO: rtt_target::rprintln!("time = {:?}", System::time().unwrap());
        System::sleep(r3::time::Duration::from_millis(900)).unwrap();
    }
}
