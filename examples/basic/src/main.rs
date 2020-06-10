#![feature(const_loop)]
#![feature(const_fn)]
#![feature(const_if_match)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
use constance::{kernel::Task, sync::Mutex};

constance_port_std::use_port!(unsafe struct System);

#[derive(Debug)]
struct Objects {
    task1: Task<System>,
    task2: Task<System>,
    mutex1: Mutex<System, u32>,
}

const COTTAGE: Objects = constance::build!(System, configure_app);

constance::configure! {
    fn configure_app(_: CfgBuilder<System>) -> Objects {
        set!(num_task_priority_levels = 4);

        let task1 = new_task! { start = task1_body, priority = 2, active = true };
        let task2 = new_task! { start = task2_body, priority = 3 };

        let mutex1 = call!(Mutex::new);

        Objects {
            task1,
            task2,
            mutex1,
        }
    }
}

fn task1_body(_: usize) {
    use constance::kernel::KernelCfg2;
    log::trace!("COTTAGE = {:#?}", COTTAGE);
    log::trace!("KENREL_STATE = {:#?}", System::state());

    COTTAGE.task2.activate().unwrap();
}

fn task2_body(_: usize) {
    dbg!();
}
