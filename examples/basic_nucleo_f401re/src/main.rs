#![feature(const_fn)]
#![feature(const_mut_refs)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]

// Install a global panic handler
use panic_rtt_target as _;

// TODO: Move this to `use_port!`
#[link_section = ".vector_table.interrupts"]
#[no_mangle]
static __INTERRUPTS: [usize; 1] = [0];

// TODO: Move this to `use_port!`
#[constance_port_arm_m::entry]
fn main() -> ! {
    rtt_target::rtt_init_print!();

    todo!()
}

/*
TODO:
use constance::{kernel::Task, prelude::*, sync::Mutex};

constance_port_arm_m::use_port!(unsafe struct System);

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
    COTTAGE.task2.activate().unwrap();
}

fn task2_body(_: usize) {
    loop {
        // TODO: dbg!(System::time().unwrap()); or something similar
        System::sleep(constance::time::Duration::from_secs(1)).unwrap();
    }
}

*/
