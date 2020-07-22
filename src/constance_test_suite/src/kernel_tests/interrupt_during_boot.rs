//! Checks that an interrupt cannot preempt the main thread.
use constance::{
    kernel::{cfg::CfgBuilder, Hunk, InterruptHandler, InterruptLine, StartupHook},
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
        StartupHook::build()
            .start(startup_hook::<System, D>)
            .finish(b);

        let int = if let [int_line, ..] = *D::INTERRUPT_LINES {
            unsafe {
                InterruptHandler::build()
                    .line(int_line)
                    .start(isr::<System, D>)
                    .unmanaged()
                    .finish(b);
            }

            Some(InterruptLine::build().line(int_line).finish(b))
        } else {
            None
        };

        let seq = Hunk::<_, SeqTracker>::build().finish(b);

        App { int, seq }
    }
}

fn startup_hook<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    assert!(System::has_cpu_lock());

    let int = if let Some(int) = D::app().int {
        int
    } else {
        log::warn!("No interrupt lines defined, skipping the test");
        D::success();
        return;
    };

    int.enable().unwrap();
    int.pend().unwrap();

    D::app().seq.expect_and_replace(1, 2);
}

fn isr<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(2, 3);
    D::success();
}
