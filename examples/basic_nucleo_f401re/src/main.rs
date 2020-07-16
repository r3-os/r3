#![feature(const_fn)]
#![feature(const_mut_refs)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]
#![cfg(target_os = "none")]
use constance::{
    kernel::{StartupHook, Task},
    prelude::*,
    sync::Mutex,
};

// Install a global panic handler that uses RTT
use panic_rtt_target as _;

constance_port_arm_m::use_port!(unsafe struct System);
constance_port_arm_m::use_systick_tickful!(unsafe impl PortTimer for System);

unsafe impl constance_port_arm_m::PortCfg for System {}

impl constance_port_arm_m::PortSysTickCfg for System {
    // SysTick = AHB/8, AHB = HSI (internal 16-MHz RC oscillator)
    const FREQUENCY: u64 = 2_000_000;
}

#[derive(Debug)]
struct Objects {
    task1: Task<System>,
    task2: Task<System>,
    mutex1: Mutex<System, u32>,
}

const COTTAGE: Objects = constance::build!(System, configure_app => Objects);

constance::configure! {
    const fn configure_app(_: &mut CfgBuilder<System>) -> Objects {
        set!(num_task_priority_levels = 4);

        // Initialize RTT (Real-Time Transfer) with a single up channel and set
        // it as the print channel for the printing macros
        new! { StartupHook<_>, start = |_| {
            rtt_target::rtt_init_print!();
        } };

        new! { StartupHook<_>, start = |_| unsafe {
            // STM32F401 specific: Keep the system clock running when
            // executing WFI. Otherise, the processor would stop
            // responding to the debug probe, severing the connection.
            let dbgmcu = &*nucleo_f401re::hal::stm32::DBGMCU::ptr();
            dbgmcu.cr.modify(|_, w| w
                .dbg_stop().set_bit()
                .dbg_sleep().set_bit()
                .dbg_standby().set_bit());
        } };

        call!(System::configure_systick);

        let task1 = new! { Task<_>, start = task1_body, priority = 2, active = true };
        let task2 = new! { Task<_>, start = task2_body, priority = 3 };

        let mutex1 = call!(Mutex::new);

        Objects {
            task1,
            task2,
            mutex1,
        }
    }
}

fn task1_body(_: usize) {
    rtt_target::rprintln!("COTTAGE = {:?}", COTTAGE);

    COTTAGE.task2.activate().unwrap();
}

fn task2_body(_: usize) {
    loop {
        rtt_target::rprintln!("time = {:?}", System::time().unwrap());
        System::sleep(constance::time::Duration::from_secs(1)).unwrap();
    }
}
