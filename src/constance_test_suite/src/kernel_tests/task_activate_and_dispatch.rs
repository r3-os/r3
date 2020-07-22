//! Activates a higher-priority task.
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
            .priority(2)
            .active(true)
            .finish(b);
        let task2 = Task::build()
            .start(task2_body::<System, D>)
            .priority(1)
            .finish(b);

        let seq = Hunk::<_, SeqTracker>::build().finish(b);

        App { task2, seq }
    }
}

fn task1_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    log::trace!("Good morning, Angel!");
    D::app().task2.activate().unwrap();

    D::app().seq.expect_and_replace(2, 3);
    D::success();
}

fn task2_body<System: Kernel, D: Driver<App<System>>>(param: usize) {
    D::app().seq.expect_and_replace(1, 2);

    log::trace!("*Rabbit noise*");

    if param == 0 {
        // Safety: There's nothing on the stack unsafe to `forget`
        unsafe { System::exit_task().unwrap() };
    }

    unreachable!();
}
