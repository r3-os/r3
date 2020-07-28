//! Sequence the execution of tasks by dynamically changing their priorities.
use constance::{
    kernel::{cfg::CfgBuilder, Hunk, Task},
    prelude::*,
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    task2: Task<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        Task::build()
            .start(task1_body::<System, D>)
            .priority(1)
            .active(true)
            .finish(b);
        let task2 = Task::build()
            .start(task2_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);

        let seq = Hunk::<_, SeqTracker>::build().finish(b);

        App { task2, seq }
    }
}

fn task1_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    // `task1` executes first because it has a higher priority.
    D::app().seq.expect_and_replace(0, 1);

    // Raise `task2`'s priority to higher than `task1`. `task2` will start
    // executing.
    D::app().task2.set_priority(0).unwrap();

    // Back from `task2`...
    D::app().seq.expect_and_replace(2, 3);

    // Exit from `task1`, relinquishing the control to `task2`.
}

fn task2_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(1, 2);

    // Reset `task2`'s priority. `task1` will resume.
    D::app().task2.set_priority(2).unwrap();

    // `task1` has exited, so `task2` is running again.
    D::app().seq.expect_and_replace(3, 4);

    D::success();
}
