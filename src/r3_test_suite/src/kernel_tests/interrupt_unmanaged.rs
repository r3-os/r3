//! Makes sure that CPU Lock doesn't mask unmanaged interrupts.
use r3::{
    hunk::Hunk,
    kernel::{prelude::*, traits, Cfg, InterruptLine, StaticInterruptHandler, StaticTask},
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
        C: ~const traits::CfgTask<System = System> + ~const traits::CfgInterruptLine,
    {
        StaticTask::define()
            .start(task_body::<System, D>)
            .priority(0)
            .active(true)
            .finish(b);

        let int = if let (&[int_line, ..], &[int_pri, ..]) =
            (D::INTERRUPT_LINES, D::UNMANAGED_INTERRUPT_PRIORITIES)
        {
            unsafe {
                StaticInterruptHandler::define()
                    .line(int_line)
                    .unmanaged()
                    .start(isr::<System, D>)
                    .finish(b);
            }

            Some(
                InterruptLine::define()
                    .line(int_line)
                    .priority(int_pri)
                    .enabled(true)
                    .finish(b),
            )
        } else {
            None
        };

        let seq = Hunk::<_, SeqTracker>::define().finish(b);

        App { int, seq }
    }
}

fn task_body<System: SupportedSystem, D: Driver<App<System>>>() {
    D::app().seq.expect_and_replace(0, 1);

    let Some(int) = D::app().int
    else {
        log::warn!(
            "No interrupt lines and compatible interrupt priorities \
            defined, skipping the test"
        );
        D::success();
        return;
    };

    System::acquire_cpu_lock().unwrap();
    D::app().seq.expect_and_replace(1, 2);
    int.pend().unwrap();
    D::app().seq.expect_and_replace(3, 4);
    unsafe { System::release_cpu_lock() }.unwrap();
    D::app().seq.expect_and_replace(4, 5);

    D::success();
}

fn isr<System: SupportedSystem, D: Driver<App<System>>>() {
    D::app().seq.expect_and_replace(2, 3);
}
