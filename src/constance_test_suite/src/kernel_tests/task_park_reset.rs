//! Checks that a park token is reset when a task is activated.
use constance::{kernel::Task, prelude::*};

use super::Driver;

pub struct App<System> {
    task2: Task<System>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            build! { Task<_>, start = task1_body::<System, D>, priority = 2, active = true };
            let task2 = build! { Task<_>, start = task2_body::<System, D>, priority = 1 };

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
    // Give a park token to itself. This should succeed in both runs because
    // the park token is reset on each run.
    D::app().task2.unpark_exact().unwrap();
}
