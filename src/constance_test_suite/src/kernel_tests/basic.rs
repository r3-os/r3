//! Runs a task at startup.
use constance::{kernel::Task, prelude::*};

use super::Driver;

#[derive(Debug)]
pub struct App<System> {
    task: Task<System>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub fn new<D: Driver<Self>>(_: CfgBuilder<System>) -> Self {
            let task = new_task! { start = task_body::<System, D>, priority = 0, active = true };

            App { task }
        }
    }
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    log::trace!("Good morning, Angel!");
    D::success();
}
