#![feature(const_loop)]
#![feature(const_if_match)]
use constance::{kernel::Task, sync::Mutex};

constance_port_std::use_port!(unsafe struct System);

#[allow(dead_code)]
struct Objects {
    task1: Task<System>,
    mutex1: Mutex<System, u32>,
}

#[allow(dead_code)]
const COTTAGE: Objects = constance::build!(System, configure_app);

constance::configure! {
    fn configure_app(_: CfgBuilder<System>) -> Objects {
        let task1 = new_task!();

        let mutex1 = call!(Mutex::new);

        Objects {
            task1,
            mutex1,
        }
    }
}