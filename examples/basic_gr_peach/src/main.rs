#![feature(const_fn_trait_bound)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_mut_refs)]
#![feature(const_trait_impl)]
#![feature(asm_sym)]
#![feature(naked_functions)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]
#![cfg(target_os = "none")]
#![recursion_limit = "1000"] // probably because of large interrupt numbers

// -----------------------------------------------------------------------

use r3_port_arm as port;
use r3_support_rza1 as support_rza1;

type System = r3_kernel::System<SystemTraits>;
port::use_port!(unsafe struct SystemTraits);
port::use_startup!(unsafe SystemTraits);
port::use_gic!(unsafe impl PortInterrupts for SystemTraits);
support_rza1::use_os_timer!(unsafe impl PortTimer for SystemTraits);

impl port::ThreadingOptions for SystemTraits {}

impl port::StartupOptions for SystemTraits {
    const MEMORY_MAP: &'static [port::MemoryMapSection] = &[
        // On-chip RAM (10MB)
        port::MemoryMapSection::new(0x2000_0000..0x20a0_0000, 0x2000_0000).with_executable(true),
        // I/O areas
        port::MemoryMapSection::new(0x3fe0_0000..0x4000_0000, 0x3fe0_0000).as_device_memory(),
        port::MemoryMapSection::new(0xe800_0000..0xe830_0000, 0xe800_0000).as_device_memory(),
        port::MemoryMapSection::new(0xfc00_0000..0xfc10_0000, 0xfc00_0000).as_device_memory(),
        port::MemoryMapSection::new(0xfcf0_0000..0xfd00_0000, 0xfcf0_0000).as_device_memory(),
    ];
}

impl port::GicOptions for SystemTraits {
    const GIC_DISTRIBUTOR_BASE: usize = 0xe8201000;
    const GIC_CPU_BASE: usize = 0xe8202000;
}

impl support_rza1::OsTimerOptions for SystemTraits {
    const FREQUENCY: u64 = 33_333_000;
}

// -----------------------------------------------------------------------

use r3::{
    kernel::{StartupHook, StaticTask},
    prelude::*,
};

// Install a global panic handler that uses the serial port
mod panic_serial;

#[derive(Debug)]
struct Objects {
    #[allow(dead_code)]
    task1: StaticTask<System>,
    task2: StaticTask<System>,
}

const COTTAGE: Objects = r3_kernel::build!(SystemTraits, configure_app => Objects);

const fn configure_app(b: &mut r3_kernel::Cfg<SystemTraits>) -> Objects {
    b.num_task_priority_levels(4);

    SystemTraits::configure_os_timer(b);

    // Initialize the serial port
    StartupHook::define()
        .start(|| {
            use support_rza1::serial::ScifExt;

            #[allow(non_snake_case)]
            let rza1::Peripherals {
                CPG, GPIO, SCIF2, ..
            } = unsafe { rza1::Peripherals::steal() };

            SCIF2.enable_clock(&CPG);
            SCIF2.configure_pins(&GPIO);
            SCIF2.configure_uart(115200);

            support_rza1::stdout::set_stdout(SCIF2.into_nb_writer());

            support_rza1::sprintln!("UART is ready");
        })
        .finish(b);

    let task1 = StaticTask::define()
        .start(task1_body)
        .priority(2)
        .active(true)
        .finish(b);
    let task2 = StaticTask::define().start(task2_body).priority(3).finish(b);

    Objects { task1, task2 }
}

fn task1_body() {
    support_rza1::sprintln!("COTTAGE = {:?}", COTTAGE);

    COTTAGE.task2.activate().unwrap();
}

fn task2_body() {
    loop {
        support_rza1::sprintln!("time = {:?}", System::time().unwrap());
        System::sleep(r3::time::Duration::from_secs(1)).unwrap();
    }
}
