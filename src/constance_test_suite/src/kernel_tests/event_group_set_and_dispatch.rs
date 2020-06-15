//! Sets an event group, waking up a task.
use constance::{
    kernel::{EventGroup, EventGroupWaitFlags, Hunk, Task},
    prelude::*,
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    eg: EventGroup<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            build! { Task<_>, start = task1_body::<System, D>, priority = 2, active = true };
            build! { Task<_>, start = task2_body::<System, D>, priority = 1, active = true };
            build! { Task<_>, start = task3_body::<System, D>, priority = 1, active = true };
            build! { Task<_>, start = task4_body::<System, D>, priority = 1, active = true };

            let eg = build! { EventGroup<_> };
            let seq = build! { Hunk<_, SeqTracker> };

            App { eg, seq }
        }
    }
}

fn task1_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(3, 4);

    D::app().eg.set(0b1111).unwrap(); // unblocks `task2`, `task3`, and `task4`

    D::app().seq.expect_and_replace(7, 8);

    assert_eq!(D::app().eg.get().unwrap(), 0b1100);

    D::success();
}

fn task2_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    D::app().eg.wait(0b01, EventGroupWaitFlags::CLEAR).unwrap(); // start waiting, switching to `task3`

    D::app().seq.expect_and_replace(4, 5);
    // unblocks `task3`
}

fn task3_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(1, 2);

    D::app().eg.wait(0b10, EventGroupWaitFlags::CLEAR).unwrap(); // start waiting, switching to `task4`

    D::app().seq.expect_and_replace(5, 6);
    // unblocks `task4`
}

fn task4_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(2, 3);

    D::app().eg.wait(0b1100, EventGroupWaitFlags::ALL).unwrap(); // start waiting, switching to `task1`

    D::app().seq.expect_and_replace(6, 7);
    // returns to `task1`
}
