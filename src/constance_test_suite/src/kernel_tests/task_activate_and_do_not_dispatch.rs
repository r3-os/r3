//! Activates a lower-priority task.
use constance::{
    kernel::{Hunk, Task},
    prelude::*,
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    task2: Task<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub fn new<D: Driver<Self>>(_: CfgBuilder<System>) -> Self {
            new_task! { start = task1_body::<System, D>, priority = 2, active = true };
            let task2 = new_task! { start = task2_body::<System, D>, priority = 3 };

            let seq = new_hunk! { SeqTracker };

            App { task2, seq }
        }
    }
}

fn task1_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    log::trace!("Good morning, Angel!");
    D::app().task2.activate().unwrap();

    // The task is already active
    assert_eq!(
        D::app().task2.activate(),
        Err(constance::kernel::ActivateTaskError::QueueOverflow)
    );

    D::app().seq.expect_and_replace(1, 2);
}

fn task2_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(2, 3);

    log::trace!("*Rabbit noise*");
    D::success();
}
