//! Sets and polls an event group in an interrupt handler.
use constance::{
    kernel::{
        cfg::CfgBuilder, EventGroup, EventGroupWaitFlags, InterruptHandler, InterruptLine,
        PollEventGroupError, StartupHook, WaitEventGroupError,
    },
    prelude::*,
};

use super::Driver;

pub struct App<System> {
    int: Option<InterruptLine<System>>,
    eg: EventGroup<System>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        let eg = EventGroup::build().finish(b);

        StartupHook::build()
            .start(startup_hook::<System, D>)
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
                    .enabled(true)
                    .priority(int_pri)
                    .finish(b),
            )
        } else {
            None
        };

        App { int, eg }
    }
}

fn startup_hook<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let int = if let Some(int) = D::app().int {
        int
    } else {
        log::warn!("No interrupt lines defined, skipping the test");
        D::success();
        return;
    };

    int.enable().unwrap();
    int.pend().unwrap();
}

fn isr<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let eg = D::app().eg;

    assert_eq!(
        eg.poll(0b1, EventGroupWaitFlags::CLEAR),
        Err(PollEventGroupError::Timeout)
    );

    eg.set(0b011).unwrap();

    eg.poll(0b110, EventGroupWaitFlags::CLEAR).unwrap();
    assert_eq!(eg.get().unwrap(), 0b001);

    // `wait` is disallowed in a non-task context
    assert_eq!(
        eg.wait(0b1, EventGroupWaitFlags::CLEAR),
        Err(WaitEventGroupError::BadContext)
    );

    D::success();
}
