//! Checks that CPU Lock is released when a task completes.
use constance::{
    kernel::{cfg::CfgBuilder, Task},
    prelude::*,
};

use super::Driver;

pub struct App<System> {
    task2: Task<System>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        Task::build()
            .start(task1_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        let task2 = Task::build()
            .start(task2_body::<System, D>)
            .priority(1)
            .finish(b);

        App { task2 }
    }
}

fn task1_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    // Activate `task2` twice
    D::app().task2.activate().unwrap();
    D::app().task2.activate().unwrap();
    D::success();
}

fn task2_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    // Acquire CPU Lock This should succeed in both runs because
    // it's automatically released on each run.
    System::acquire_cpu_lock().unwrap();
}
