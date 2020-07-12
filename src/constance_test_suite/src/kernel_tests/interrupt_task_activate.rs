//! Checks the return codes of disallowed system calls made in an interrupt
//! context.
//! TODO: wrong
use constance::{
    kernel::{InterruptHandler, InterruptLine, Task},
    prelude::*,
};

use super::Driver;

pub struct App<System> {
    task2: Task<System>,
    int: Option<InterruptLine<System>>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            new! { Task<_>, start = task_body1::<System, D>, priority = 1, active = true };
            let task2 = new! { Task<_>, start = task_body2::<System, D>, priority = 0 };

            let int = if let [int_line, ..] = *D::INTERRUPT_LINES {
                unsafe {
                    new! { InterruptHandler<_>,
                        line = int_line, start = isr::<System, D>, unmanaged };
                }

                Some(new! { InterruptLine<_>, line = int_line, enabled = true })
            } else {
                None
            };

            App { task2, int }
        }
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
