//! Sets and polls an event group in an interrupt handler.
use r3::kernel::{
    prelude::*, traits, Cfg, EventGroupWaitFlags, InterruptLine, PollEventGroupError, StartupHook,
    StaticEventGroup, StaticInterruptHandler, WaitEventGroupError,
};

use super::Driver;

pub trait SupportedSystem:
    traits::KernelBase + traits::KernelEventGroup + traits::KernelInterruptLine
{
}
impl<T: traits::KernelBase + traits::KernelEventGroup + traits::KernelInterruptLine> SupportedSystem
    for T
{
}

pub struct App<System: SupportedSystem> {
    int: Option<InterruptLine<System>>,
    eg: StaticEventGroup<System>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System>
            + ~const traits::CfgTask
            + ~const traits::CfgEventGroup
            + ~const traits::CfgInterruptLine,
    {
        let eg = StaticEventGroup::define().finish(b);

        StartupHook::define()
            .start(startup_hook::<System, D>)
            .finish(b);

        let int = if let (&[int_line, ..], &[int_pri, ..]) =
            (D::INTERRUPT_LINES, D::INTERRUPT_PRIORITIES)
        {
            StaticInterruptHandler::define()
                .line(int_line)
                .start(isr::<System, D>)
                .finish(b);

            Some(
                InterruptLine::define()
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

fn startup_hook<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
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

fn isr<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
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
