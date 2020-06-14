//! Runs a task at startup.
use constance::{kernel::Task, prelude::*};

use super::Driver;

#[derive(Debug)]
pub struct App<System> {
    task: Task<System>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            let task = build! { Task<_>, start = task_body::<System, D>, priority = 0, active = true };

            App { task }
        }
    }
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    log::trace!("Good morning, Angel!");
    D::success();
}
