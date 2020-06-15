//! Asserts that tasks in the same ready queue are processed in a FIFO order.
use constance::{
    kernel::{Hunk, Task},
    prelude::*,
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    task2: Task<System>,
    task3: Task<System>,
    task4: Task<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            build! { Task<_>, start = task1_body::<System, D>, priority = 2, active = true };
            let task2 = build! { Task<_>, start = task2_body::<System, D>, priority = 2, param = 2 };
            let task3 = build! { Task<_>, start = task2_body::<System, D>, priority = 2, param = 3 };
            let task4 = build! { Task<_>, start = task2_body::<System, D>, priority = 2, param = 4 };

            let seq = build! { Hunk<_, SeqTracker> };

            App { task2, task3, task4, seq }
        }
    }
}

fn task1_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    log::trace!("Good morning, Angel!");
    D::app().task2.activate().unwrap();
    D::app().task3.activate().unwrap();
    D::app().task4.activate().unwrap();

    D::app().seq.expect_and_replace(1, 2);
}

fn task2_body<System: Kernel, D: Driver<App<System>>>(i: usize) {
    D::app().seq.expect_and_replace(i, i + 1);
    log::trace!("*Rabbit noise {}*", i);
    if i == 4 {
        D::success();
    }
}
