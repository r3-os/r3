#![feature(const_fn)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_mut_refs)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]
#![cfg(target_os = "none")]
use constance::{
    kernel::{cfg::CfgBuilder, StartupHook, Task},
    prelude::*,
    sync::Mutex,
};
use constance_port_arm_m as port;

// Install a global panic handler that uses RTT
use panic_rtt_target as _;

port::use_port!(unsafe struct System);
port::use_systick_tickful!(unsafe impl PortTimer for System);

impl port::ThreadingOptions for System {
    // Disable the use of WFI because it breaks RTT and debugger connection
    const USE_WFI: bool = false;
}

impl port::SysTickOptions for System {
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

const fn configure_app(b: &mut CfgBuilder<System>) -> Objects {
    b.num_task_priority_levels(4);

    // Initialize RTT (Real-Time Transfer) with a single up channel and set
    // it as the print channel for the printing macros
    StartupHook::build()
        .start(|_| {
            rtt_target::rtt_init_print!();
        })
        .finish(b);

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
    rtt_target::rprintln!("COTTAGE = {:?}", COTTAGE);

    COTTAGE.task2.activate().unwrap();
}

fn task2_body(_: usize) {
    loop {
        rtt_target::rprintln!("time = {:?}", System::time().unwrap());
        System::sleep(constance::time::Duration::from_secs(1)).unwrap();
    }
}
