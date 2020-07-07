//! Validates error codes returned by interrupt line manipulation methods. Also,
//! checks miscellaneous properties of interrupt lines.
//! TODO: This description is wrong
use constance::{
    kernel::{Hunk, InterruptHandler, InterruptLine, Task},
    prelude::*,
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    int: [Option<InterruptLine<System>>; 2],
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            new! { Task<_>, start = task_body::<System, D>, priority = 0, active = true };

            let int = [
                if D::INTERRUPT_LINES.len() >= 1 {
                    let int_line = D::INTERRUPT_LINES[0];
                    let pri = D::INTERRUPT_PRIORITY_HIGH;
                    new! { InterruptHandler<_>, line = int_line, start = isr0::<System, D> };
                    Some(new! { InterruptLine<_>, line = int_line, priority = pri, enabled = true })
                } else {
                    None
                },
                if D::INTERRUPT_LINES.len() >= 2 {
                    let int_line = D::INTERRUPT_LINES[1];
                    let pri = D::INTERRUPT_PRIORITY_LOW;
                    new! { InterruptHandler<_>, line = int_line, start = isr1::<System, D> };
                    Some(new! { InterruptLine<_>, line = int_line, priority = pri, enabled = true })
                } else {
                    None
                },
            ];

            let seq = new! { Hunk<_, SeqTracker> };

            App { int, seq }
        }
    }
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    // Pend both interrupts at the same time. Regardless the order of reception,
    // the higher-priority one should be handled first
    System::acquire_cpu_lock().unwrap();
    if let Some(int) = D::app().int[1] {
        int.pend().unwrap();
    }
    if let Some(int) = D::app().int[0] {
        int.pend().unwrap();
    }
    unsafe { System::release_cpu_lock() }.unwrap();

    if let [None, None] = D::app().int {
        log::warn!("No interrupt lines defined, skipping the test");
        D::success();
        return;
    }
}

fn isr1<System: Kernel, D: Driver<App<System>>>(_: usize) {
    log::trace!("isr1");

    D::app().seq.expect_and_replace(2, 3);

    D::success();
}

fn isr0<System: Kernel, D: Driver<App<System>>>(_: usize) {
    log::trace!("isr0");

    D::app().seq.expect_and_replace(1, 2);

    if D::app().int[1].is_none() {
        log::warn!("Only one interrupt line is defined, skipping the second part of the test");
        D::success();
        return;
    }
}
