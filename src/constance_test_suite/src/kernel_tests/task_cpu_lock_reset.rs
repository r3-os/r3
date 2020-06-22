//! Checks that CPU Lock is released when a task completes.
use constance::{kernel::Task, prelude::*};

use super::Driver;

pub struct App<System> {
    task2: Task<System>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            new! { Task<_>, start = task1_body::<System, D>, priority = 2, active = true };
            let task2 = new! { Task<_>, start = task2_body::<System, D>, priority = 1 };

            App { task2 }
        }
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
