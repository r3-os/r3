//! Make sure interrupt handlers are called in the ascending order of priority.
use r3::{
    hunk::Hunk,
    kernel::{traits, Cfg, InterruptLine, StaticInterruptHandler, StaticTask},
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
            (D::INTERRUPT_LINES, D::INTERRUPT_PRIORITIES)
        {
            StaticInterruptHandler::define()
                .line(int_line)
                .start((3, isr::<System, D>))
                .priority(30)
                .finish(b);
            StaticInterruptHandler::define()
                .line(int_line)
                .start((1, isr::<System, D>))
                .priority(10)
                .finish(b);
            StaticInterruptHandler::define()
                .line(int_line)
                .start((7, isr::<System, D>))
                .priority(70)
                .finish(b);
            StaticInterruptHandler::define()
                .line(int_line)
                .start((5, isr::<System, D>))
                .priority(50)
                .finish(b);
            StaticInterruptHandler::define()
                .line(int_line)
                .start((4, isr::<System, D>))
                .priority(40)
                .finish(b);
            StaticInterruptHandler::define()
                .line(int_line)
                .start((10, isr::<System, D>))
                .priority(100)
                .finish(b);
            StaticInterruptHandler::define()
                .line(int_line)
                .start((9, isr::<System, D>))
                .priority(90)
                .finish(b);
            StaticInterruptHandler::define()
                .line(int_line)
                .start((8, isr::<System, D>))
                .priority(80)
                .finish(b);
            StaticInterruptHandler::define()
                .line(int_line)
                .start((2, isr::<System, D>))
                .priority(20)
                .finish(b);
            StaticInterruptHandler::define()
                .line(int_line)
                .start((6, isr::<System, D>))
                .priority(60)
                .finish(b);

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

    let int = if let Some(int) = D::app().int {
        int
    } else {
        log::warn!("No interrupt lines defined, skipping the test");
        D::success();
        return;
    };

    int.pend().unwrap();
}

fn isr<System: SupportedSystem, D: Driver<App<System>>>(i: usize) {
    log::trace!("isr({})", i);
    D::app().seq.expect_and_replace(i, i + 1);

    if i == 10 {
        D::success();
    }
}
