//! Checks that an interrupt cannot preempt the main thread.
use r3::{
    hunk::Hunk,
    kernel::{prelude::*, traits, Cfg, InterruptLine, StartupHook, StaticInterruptHandler},
};

use super::Driver;
use crate::utils::SeqTracker;

pub trait SupportedSystem:
    traits::KernelBase + traits::KernelInterruptLine + traits::KernelStatic
{
}
impl<T: traits::KernelBase + traits::KernelInterruptLine + traits::KernelStatic> SupportedSystem
    for T
{
}

pub struct App<System: SupportedSystem> {
    int: Option<InterruptLine<System>>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System>
            + ~const traits::CfgTask
            + ~const traits::CfgInterruptLine,
    {
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
                    .priority(int_pri)
                    .finish(b),
            )
        } else {
            None
        };

        let seq = Hunk::<_, SeqTracker>::define().finish(b);

        App { int, seq }
    }
}

fn startup_hook<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
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

fn isr<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(2, 3);
    D::success();
}
