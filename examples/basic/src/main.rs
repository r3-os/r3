#![feature(const_loop)]
#![feature(const_if_match)]
use constance::{kernel::Task, sync::Mutex};

struct System;

struct Objects {
    task1: Task<System>,
    mutex1: Mutex<System, u32>,
}

constance::configure! {
    fn configure_app(ctx: CfgBuilder<System>) -> Objects {
        let task1 = constance::create_task!(ctx);

        let mutex1 = constance::call!(ctx, constance::sync::Mutex::new);

        Objects {
            task1,
            mutex1,
        }
    }
}

const ID: Objects = constance::build!(System, configure_app);
constance_port_std::use_port!(unsafe System);
