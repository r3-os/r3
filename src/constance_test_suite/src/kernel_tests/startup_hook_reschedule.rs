//! Checks that a startup hook can alter the task scheduling.
use constance::{
    kernel::{Hunk, StartupHook, Task},
    prelude::*,
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    task1: Task<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            let task1 = new! { Task<_>, start = task1_body::<System, D>, priority = 0 };
            new! { Task<_>, start = task2_body::<System, D>, priority = 4, active = true };

            new! { StartupHook<_>, start = hook::<System, D> };

            let seq = new! { Hunk<_, SeqTracker> };

            App { task1, seq }
        }
    }
}

fn task1_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(2, 3);
}

fn task2_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(3, 4);
    D::success();
}

fn hook<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    D::app().task1.activate().unwrap();
    // now `task1` should run first

    D::app().seq.expect_and_replace(1, 2);
}
