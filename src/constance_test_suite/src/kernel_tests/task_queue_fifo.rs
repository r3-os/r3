//! Asserts that tasks in the same ready queue are processed in a FIFO order.
use constance::{
    hunk::Hunk,
    kernel::{cfg::CfgBuilder, Task},
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
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        Task::build()
            .start(task1_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        let task2 = Task::build()
            .start(task2_body::<System, D>)
            .priority(2)
            .param(2)
            .finish(b);
        let task3 = Task::build()
            .start(task2_body::<System, D>)
            .priority(2)
            .param(3)
            .finish(b);
        let task4 = Task::build()
            .start(task2_body::<System, D>)
            .priority(2)
            .param(4)
            .finish(b);

        let seq = Hunk::<_, SeqTracker>::build().finish(b);

        App {
            task2,
            task3,
            task4,
            seq,
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
