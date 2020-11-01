//! Checks the return codes of disallowed system calls made in an interrupt
//! context.
//! TODO: wrong
use r3::{
    kernel::{cfg::CfgBuilder, InterruptHandler, InterruptLine, Task},
    prelude::*,
};

use super::Driver;

pub struct App<System> {
    task2: Task<System>,
    int: Option<InterruptLine<System>>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        Task::build()
            .start(task_body1::<System, D>)
            .priority(1)
            .active(true)
            .finish(b);
        let task2 = Task::build()
            .start(task_body2::<System, D>)
            .priority(0)
            .finish(b);

        let int = if let (&[int_line, ..], &[int_pri, ..]) =
            (D::INTERRUPT_LINES, D::INTERRUPT_PRIORITIES)
        {
            InterruptHandler::build()
                .line(int_line)
                .start(isr::<System, D>)
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

        App { task2, int }
    }
}

fn task_body1<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let int = if let Some(int) = D::app().int {
        int
    } else {
        log::warn!("No interrupt lines defined, skipping the test");
        D::success();
        return;
    };

    int.pend().unwrap();
}

fn isr<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().task2.activate().unwrap();
}

fn task_body2<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::success();
}
