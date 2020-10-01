#![feature(const_fn)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_mut_refs)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
use constance::{
    kernel::{cfg::CfgBuilder, Task},
    prelude::*,
    sync::Mutex,
};

constance_port_std::use_port!(unsafe struct System);

#[derive(Debug)]
struct Objects {
    task1: Task<System>,
    task2: Task<System>,
    mutex1: Mutex<System, u32>,
}

const COTTAGE: Objects = constance::build!(System, configure_app => Objects);

const fn configure_app(b: &mut CfgBuilder<System>) -> Objects {
    b.num_task_priority_levels(4);

    let task1 = Task::build()
        .start(task1_body)
        .priority(2)
        .active(true)
        .finish(b);
    let task2 = Task::build().start(task2_body).priority(3).finish(b);

    let mutex1 = Mutex::new(b);

    Objects {
        task1,
        task2,
        mutex1,
    }
}

fn task1_body(_: usize) {
    use constance::kernel::KernelCfg2;
    log::trace!("COTTAGE = {:#?}", COTTAGE);
    log::trace!("KENREL_STATE = {:#?}", System::state());

    COTTAGE.task2.activate().unwrap();
}

fn task2_body(_: usize) {
    loop {
        dbg!(System::time().unwrap());
        System::sleep(constance::time::Duration::from_secs(1)).unwrap();
    }
}
