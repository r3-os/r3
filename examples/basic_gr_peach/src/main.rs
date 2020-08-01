#![feature(const_fn)]
#![feature(const_mut_refs)]
#![feature(llvm_asm)]
#![feature(naked_functions)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]
#![cfg(target_os = "none")]

// -----------------------------------------------------------------------

use constance_port_arm as port;

port::use_port!(unsafe struct System);
port::use_startup!(unsafe System);
port::use_gic!(unsafe impl PortInterrupts for System);

impl port::ThreadingOptions for System {}

impl constance::kernel::PortTimer for System {
    // TODO
    const MAX_TICK_COUNT: constance::kernel::UTicks = 0xffffffff;
    const MAX_TIMEOUT: constance::kernel::UTicks = 0x80000000;
    unsafe fn tick_count() -> constance::kernel::UTicks {
        0
    }
}

// -----------------------------------------------------------------------

use constance::{
    kernel::{cfg::CfgBuilder, StartupHook, Task},
    prelude::*,
};

// Install a global panic handler that uses RTT
mod panic_rtt_target;

#[derive(Debug)]
struct Objects {
    task1: Task<System>,
    task2: Task<System>,
}

const COTTAGE: Objects = constance::build!(System, configure_app => Objects);

const fn configure_app(b: &mut CfgBuilder<System>) -> Objects {
    b.num_task_priority_levels(4);

    // Initialize RTT (Real-Time Transfer) with a single up channel and set
    // it as the print channel for the printing macros
    StartupHook::build()
        .start(|_| {
            let channels = rtt_target::rtt_init! {
                up: {
                    0: {
                        size: 1024
                        mode: NoBlockSkip
                        name: "Terminal"
                    }
                }
            };

            unsafe {
                rtt_target::set_print_channel_cs(
                    channels.up.0,
                    &((|arg, f| f(arg)) as rtt_target::CriticalSectionFunc),
                )
            };

            rtt_target::rprintln!("RTT is ready");
        })
        .finish(b);

    let task1 = Task::build()
        .start(task1_body)
        .priority(2)
        .active(true)
        .finish(b);
    let task2 = Task::build().start(task2_body).priority(3).finish(b);

    Objects { task1, task2 }
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
