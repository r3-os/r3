#![feature(const_loop)]
#![feature(const_if_match)]

struct System;

struct Objects {
    task1: constance::kernel::Task<System>,
}

constance::configure! {
    fn configure_app(ctx: CfgBuilder<System>) -> Objects {
        let task1 = constance::create_task!(ctx);
        Objects {
            task1
        }
    }
}

const ID: Objects = constance::build!(System, configure_app);
constance_port_std::use_port!(unsafe System);
