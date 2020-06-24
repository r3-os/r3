//! Checks the return codes of disallowed system calls made in an interrupt
//! context.
use constance::{
    kernel::{self, InterruptHandler, InterruptLine, Task},
    prelude::*,
};

use super::Driver;

#[derive(Debug)]
pub struct App<System> {
    int: Option<InterruptLine<System>>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            new! { Task<_>, start = task_body::<System, D>, priority = 0, active = true };

            let int = if let [int_line, ..] = *D::INTERRUPT_LINES {
                unsafe {
                    new! { InterruptHandler<_>,
                        line = int_line, start = isr::<System, D>, unmanaged };
                }

                Some(new! { InterruptLine<_>, line = int_line, enabled = true })
            } else {
                None
            };

            App { int }
        }
    }
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
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
    // Disallowed in a non-task context
    assert_eq!(
        D::app().int.unwrap().set_priority(1),
        Err(kernel::SetInterruptLinePriorityError::BadContext),
    );
    assert_eq!(
        unsafe { D::app().int.unwrap().set_priority_unchecked(1) },
        Err(kernel::SetInterruptLinePriorityError::BadContext),
    );
    assert_eq!(
        System::boost_priority(),
        Err(kernel::BoostPriorityError::BadContext),
    );

    D::success();
}
