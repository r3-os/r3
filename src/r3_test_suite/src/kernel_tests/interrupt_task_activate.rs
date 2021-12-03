//! Checks the return codes of disallowed system calls made in an interrupt
//! context.
//! TODO: wrong
use r3::kernel::{traits, Cfg, InterruptHandler, InterruptLine, Task};

use super::Driver;

pub trait SupportedSystem: traits::KernelBase + traits::KernelInterruptLine {}
impl<T: traits::KernelBase + traits::KernelInterruptLine> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    task2: Task<System>,
    int: Option<InterruptLine<System>>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System>
            + ~const traits::CfgTask
            + ~const traits::CfgInterruptLine,
    {
        Task::define()
            .start(task_body1::<System, D>)
            .priority(1)
            .active(true)
            .finish(b);
        let task2 = Task::define()
            .start(task_body2::<System, D>)
            .priority(0)
            .finish(b);

        let int = if let (&[int_line, ..], &[int_pri, ..]) =
            (D::INTERRUPT_LINES, D::INTERRUPT_PRIORITIES)
        {
            InterruptHandler::define()
                .line(int_line)
                .start(isr::<System, D>)
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

        App { task2, int }
    }
}

fn task_body1<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    let int = if let Some(int) = D::app().int {
        int
    } else {
        log::warn!("No interrupt lines defined, skipping the test");
        D::success();
        return;
    };

    int.pend().unwrap();
}

fn isr<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::app().task2.activate().unwrap();
}

fn task_body2<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::success();
}
