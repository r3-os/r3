#![feature(const_fn_fn_ptr_basics)]
#![feature(const_fn_trait_bound)]
#![feature(const_refs_to_cell)]
#![feature(const_trait_impl)]
#![feature(const_mut_refs)]
#![feature(asm_sym)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]
#![cfg(target_os = "none")]
use r3::{
    kernel::{StartupHook, StaticTask},
    prelude::*,
    sync::StaticMutex,
};
use r3_port_arm_m as port;

// Install a global panic handler that uses RTT
use panic_rtt_target as _;

type System = r3_kernel::System<SystemTraits>;
port::use_port!(unsafe struct SystemTraits);
port::use_rt!(unsafe SystemTraits);
port::use_systick_tickful!(unsafe impl PortTimer for SystemTraits);

impl port::ThreadingOptions for SystemTraits {
    // Disable the use of WFI because it breaks RTT and debugger connection
    const USE_WFI: bool = false;
}

impl port::SysTickOptions for SystemTraits {
    // SysTick = AHB/8, AHB = HSI (internal 16-MHz RC oscillator)
    const FREQUENCY: u64 = 2_000_000;
}

#[derive(Debug)]
struct Objects {
    _task1: StaticTask<System>,
    task2: StaticTask<System>,
    _mutex1: StaticMutex<System, u32>,
}

const COTTAGE: Objects = r3_kernel::build!(SystemTraits, configure_app => Objects);

const fn configure_app(b: &mut r3_kernel::Cfg<SystemTraits>) -> Objects {
    b.num_task_priority_levels(4);

    // Initialize RTT (Real-Time Transfer) with a single up channel and set
    // it as the print channel for the printing macros
    StartupHook::define()
        .start(|| {
            rtt_target::rtt_init_print!();
        })
        .finish(b);

    SystemTraits::configure_systick(b);

    let task1 = StaticTask::define()
        .start(task1_body)
        .priority(2)
        .active(true)
        .finish(b);
    let task2 = StaticTask::define().start(task2_body).priority(3).finish(b);

    let mutex1 = StaticMutex::define().finish(b);

    Objects {
        _task1: task1,
        task2,
        _mutex1: mutex1,
    }
}

fn task1_body() {
    rtt_target::rprintln!("COTTAGE = {:?}", COTTAGE);

    COTTAGE.task2.activate().unwrap();
}

fn task2_body() {
    loop {
        rtt_target::rprintln!("time = {:?}", System::time().unwrap());
        System::sleep(r3::time::Duration::from_secs(1)).unwrap();
    }
}
