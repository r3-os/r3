//! Checks the return codes of disallowed system calls made in an interrupt
//! context.
//! TODO: wrong
use r3::kernel::{prelude::*, traits, Cfg, InterruptLine, StaticInterruptHandler, StaticTask};

use super::Driver;

pub trait SupportedSystem: traits::KernelBase + traits::KernelInterruptLine {}
impl<T: traits::KernelBase + traits::KernelInterruptLine> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    task2: StaticTask<System>,
    int: Option<InterruptLine<System>>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgTask<System = System> + ~const traits::CfgInterruptLine,
    {
        StaticTask::define()
            .start(task_body1::<System, D>)
            .priority(1)
            .active(true)
            .finish(b);
        let task2 = StaticTask::define()
            .start(task_body2::<System, D>)
            .priority(0)
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
                    .enabled(true)
                    .finish(b),
            )
        } else {
            None
        };

        App { task2, int }
    }
}

fn task_body1<System: SupportedSystem, D: Driver<App<System>>>() {
    let Some(int) = D::app().int 
    else {
        log::warn!("No interrupt lines defined, skipping the test");
        D::success();
        return;
    };

    int.pend().unwrap();
}

fn isr<System: SupportedSystem, D: Driver<App<System>>>() {
    D::app().task2.activate().unwrap();
}

fn task_body2<System: SupportedSystem, D: Driver<App<System>>>() {
    D::success();
}
