//! Makes sure that CPU Lock doesn't mask unmanaged interrupts.
use r3::{
    hunk::Hunk,
    kernel::{cfg::CfgBuilder, InterruptHandler, InterruptLine, Task},
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
            (D::INTERRUPT_LINES, D::UNMANAGED_INTERRUPT_PRIORITIES)
        {
            unsafe {
                InterruptHandler::build()
                    .line(int_line)
                    .unmanaged()
                    .start(isr::<System, D>)
                    .finish(b);
            }

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

    if let Some(int) = D::app().int {
        System::acquire_cpu_lock().unwrap();
        D::app().seq.expect_and_replace(1, 2);
        int.pend().unwrap();
        D::app().seq.expect_and_replace(3, 4);
        unsafe { System::release_cpu_lock() }.unwrap();
        D::app().seq.expect_and_replace(4, 5);
    } else {
        log::warn!(
            "No interrupt lines and compatible interrupt priorities \
            defined, skipping the test"
        );
    }

    D::success();
}

fn isr<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(2, 3);
}
