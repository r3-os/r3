#![feature(const_loop)]
#![feature(const_fn)]
#![feature(const_if_match)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
use constance::{kernel::Task, sync::Mutex};

constance_port_std::use_port!(unsafe struct System);

#[allow(dead_code)]
struct Objects {
    task1: Task<System>,
    task2: Task<System>,
    mutex1: Mutex<System, u32>,
}

#[allow(dead_code)]
const COTTAGE: Objects = constance::build!(System, configure_app);

constance::configure! {
    fn configure_app(_: CfgBuilder<System>) -> Objects {
        let task1 = new_task! { start = task1_body };
        let task2 = new_task! { start = task2_body };

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
    dbg!();
}
