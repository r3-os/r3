//! Make sure interrupt handlers are called in the ascending order of priority.
use constance::{
    kernel::{cfg::CfgBuilder, Hunk, InterruptHandler, InterruptLine, Task},
    prelude::*,
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    int: Option<InterruptLine<System>>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        Task::build()
            .start(task_body::<System, D>)
            .priority(0)
            .active(true)
            .finish(b);

        let int = if let (&[int_line, ..], &[int_pri, ..]) =
            (D::INTERRUPT_LINES, D::INTERRUPT_PRIORITIES)
        {
            InterruptHandler::build()
                .line(int_line)
                .start(isr::<System, D>)
                .param(3)
                .priority(30)
                .finish(b);
            InterruptHandler::build()
                .line(int_line)
                .start(isr::<System, D>)
                .param(1)
                .priority(10)
                .finish(b);
            InterruptHandler::build()
                .line(int_line)
                .start(isr::<System, D>)
                .param(7)
                .priority(70)
                .finish(b);
            InterruptHandler::build()
                .line(int_line)
                .start(isr::<System, D>)
                .param(5)
                .priority(50)
                .finish(b);
            InterruptHandler::build()
                .line(int_line)
                .start(isr::<System, D>)
                .param(4)
                .priority(40)
                .finish(b);
            InterruptHandler::build()
                .line(int_line)
                .start(isr::<System, D>)
                .param(10)
                .priority(100)
                .finish(b);
            InterruptHandler::build()
                .line(int_line)
                .start(isr::<System, D>)
                .param(9)
                .priority(90)
                .finish(b);
            InterruptHandler::build()
                .line(int_line)
                .start(isr::<System, D>)
                .param(8)
                .priority(80)
                .finish(b);
            InterruptHandler::build()
                .line(int_line)
                .start(isr::<System, D>)
                .param(2)
                .priority(20)
                .finish(b);
            InterruptHandler::build()
                .line(int_line)
                .start(isr::<System, D>)
                .param(6)
                .priority(60)
                .finish(b);

            Some(
                InterruptLine::build()
                    .line(int_line)
                    .priority(int_pri)
                    .enabled(true)
                    .finish(b),
            )
        } else {
            None
        };

        let seq = Hunk::<_, SeqTracker>::build().finish(b);

        App { int, seq }
    }
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    let int = if let Some(int) = D::app().int {
        int
    } else {
        log::warn!("No interrupt lines defined, skipping the test");
        D::success();
        return;
    };

    int.pend().unwrap();
}

fn isr<System: Kernel, D: Driver<App<System>>>(i: usize) {
    log::trace!("isr({})", i);
    D::app().seq.expect_and_replace(i, i + 1);

    if i == 10 {
        D::success();
    }
}
