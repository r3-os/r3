//! Executes
use constance::{
    kernel::{cfg::CfgBuilder, StartupHook, Task},
    prelude::*,
};
use constance_test_suite::kernel_tests::Driver;
use core::marker::PhantomData;

pub struct App<System> {
    _phantom: PhantomData<System>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        StartupHook::build()
            .start(startup_hook_body::<System, D>)
            .finish(b);

        Task::build()
            .start(task_body::<System, D>)
            .priority(0)
            .active(true)
            .finish(b);

        App {
            _phantom: PhantomData,
        }
    }
}

fn startup_hook_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    log::debug!("calling do_test from a startup hook");
    do_test();
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    log::debug!("calling do_test from a task");
    do_test();
    D::success();
}

fn do_test() {
    // TODO
}
