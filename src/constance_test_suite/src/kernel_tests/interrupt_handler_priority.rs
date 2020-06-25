//! Make sure interrupt handlers are called in the ascending order of priority.
use constance::{
    kernel::{Hunk, InterruptHandler, InterruptLine, Task},
    prelude::*,
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    int: Option<InterruptLine<System>>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            new! { Task<_>, start = task_body::<System, D>, priority = 0, active = true };

            let int = if let [int_line, ..] = *D::INTERRUPT_LINES {
                new! { InterruptHandler<_>, line = int_line, start = isr::<System, D>,
                    param = 3, priority = 30 };
                new! { InterruptHandler<_>, line = int_line, start = isr::<System, D>,
                    param = 1, priority = 10 };
                new! { InterruptHandler<_>, line = int_line, start = isr::<System, D>,
                    param = 7, priority = 70 };
                new! { InterruptHandler<_>, line = int_line, start = isr::<System, D>,
                    param = 5, priority = 50 };
                new! { InterruptHandler<_>, line = int_line, start = isr::<System, D>,
                    param = 4, priority = 40 };
                new! { InterruptHandler<_>, line = int_line, start = isr::<System, D>,
                    param = 10, priority = 100 };
                new! { InterruptHandler<_>, line = int_line, start = isr::<System, D>,
                    param = 9, priority = 90 };
                new! { InterruptHandler<_>, line = int_line, start = isr::<System, D>,
                    param = 8, priority = 80 };
                new! { InterruptHandler<_>, line = int_line, start = isr::<System, D>,
                    param = 2, priority = 20 };
                new! { InterruptHandler<_>, line = int_line, start = isr::<System, D>,
                    param = 6, priority = 60 };

                Some(new! { InterruptLine<_>,
                    line = int_line, priority = D::INTERRUPT_PRIORITY_HIGH, enabled = true })
            } else {
                None
            };

            let seq = new! { Hunk<_, SeqTracker> };

            App { int, seq }
        }
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
