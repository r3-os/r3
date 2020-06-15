//! Interrupts a task waiting for an event bit to be set.
//!
//! 1. (`seq`: 0 → 1) `task0` activates `task[1-4]` in a particular order.
//! 2. (`seq`: 1 → 5) `task[1-4]` start waiting for a event bit to be set.
//! 3. (`seq`: 5 → 9) `task0` sets the event bit for four times. `task[1-4]`
//!    should be unblocked in the same order.
//!
use constance::{
    kernel::{EventGroup, EventGroupWaitFlags, Hunk, QueueOrder, Task, WaitEventGroupError},
    prelude::*,
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    eg: EventGroup<System>,
    task1: Task<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            build! { Task<_>, start = task0_body::<System, D>, priority = 2, active = true };
            let task1 = build! { Task<_>, start = task1_body::<System, D>, priority = 1, active = true };

            let eg = build! { EventGroup<_>, queue_order = QueueOrder::Fifo };
            let seq = build! { Hunk<_, SeqTracker> };

            App { eg, task1, seq }
        }
    }
}

fn task0_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(1, 2);
    D::app().task1.interrupt().unwrap();
}

fn task1_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    assert_eq!(
        // start waiting, switching to `task0`
        D::app().eg.wait(0b1, EventGroupWaitFlags::CLEAR),
        // ... the control is returned when `task0` interrupts `task1`
        Err(WaitEventGroupError::Interrupted),
    );

    D::app().seq.expect_and_replace(2, 3);

    // Enqueue an interupt request
    D::app().task1.interrupt().unwrap();

    assert_eq!(
        D::app().eg.wait(0b1, EventGroupWaitFlags::CLEAR),
        // this will not block becasue an interrupt request is enqueued
        Err(WaitEventGroupError::Interrupted),
    );

    // There are no interrupt requests enqueued at this point
    assert_eq!(D::app().task1.cancel_interrupt(), Ok(0));

    D::success();
}
