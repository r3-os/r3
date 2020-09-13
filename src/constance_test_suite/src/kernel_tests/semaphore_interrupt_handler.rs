//! Signals a semaphore in an interrupt handler, waking up a task.
use constance::{
    kernel::{cfg::CfgBuilder, Hunk, InterruptHandler, InterruptLine, Semaphore, Task},
    prelude::*,
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    int: Option<InterruptLine<System>>,
    sem: Semaphore<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        Task::build()
            .start(task1_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        Task::build()
            .start(task2_body::<System, D>)
            .priority(1)
            .active(true)
            .finish(b);

        let sem = Semaphore::build().initial(0).maximum(2).finish(b);
        let seq = Hunk::<_, SeqTracker>::build().finish(b);

        let int = if let [int_line, ..] = *D::INTERRUPT_LINES {
            InterruptHandler::build()
                .line(int_line)
                .start(isr::<System, D>)
                .finish(b);

            Some(
                InterruptLine::build()
                    .line(int_line)
                    .enabled(true)
                    .priority(D::INTERRUPT_PRIORITY_HIGH)
                    .finish(b),
            )
        } else {
            None
        };

        App { sem, seq, int }
    }
}

fn task1_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(1, 2);

    let int = if let Some(int) = D::app().int {
        int
    } else {
        log::warn!("No interrupt lines defined, skipping the test");
        D::success();
        return;
    };

    int.pend().unwrap();
}

fn task2_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    D::app().sem.wait().unwrap(); // start waiting, switching to `task1`

    D::app().seq.expect_and_replace(3, 4);

    assert_eq!(D::app().sem.get().unwrap(), 0);

    D::success();
}

fn isr<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let sem = D::app().sem;

    D::app().seq.expect_and_replace(2, 3);

    assert_eq!(
        sem.poll(),
        Err(constance::kernel::PollSemaphoreError::Timeout)
    );
    assert_eq!(
        sem.wait(),
        Err(constance::kernel::WaitSemaphoreError::BadContext)
    );

    sem.signal(1).unwrap(); // wakes up `task2`
}
