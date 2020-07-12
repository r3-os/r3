//! Checks that an interrupt can preempt the main thread.
use constance::{
    kernel::{Hunk, InterruptHandler, InterruptLine, StartupHook, Task},
    prelude::*,
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    int: Option<InterruptLine<System>>,
    task: Task<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            let task = new! { Task<_>, start = task_body::<System, D>, priority = 0 };

            new! { StartupHook<_>, start = startup_hook::<System, D> };

            let int = if let [int_line, ..] = *D::INTERRUPT_LINES {
                unsafe {
                    new! { InterruptHandler<_>,
                        line = int_line, start = isr::<System, D>, unmanaged };
                }

                Some(new! { InterruptLine<_>, line = int_line })
            } else {
                None
            };

            let seq = new! { Hunk<_, SeqTracker> };

            App { int, task, seq }
        }
    }
}

fn startup_hook<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    let int = if let Some(int) = D::app().int {
        int
    } else {
        log::warn!("No interrupt lines defined, skipping the test");
        D::app().seq.expect_and_replace(1, 3);
        D::app().task.activate().unwrap();
        return;
    };

    int.enable().unwrap();
    int.pend().unwrap();

    D::app().seq.expect_and_replace(2, 3);

    D::app().task.activate().unwrap();
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(3, 4);
    D::success();
}

fn isr<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(1, 2);
}
