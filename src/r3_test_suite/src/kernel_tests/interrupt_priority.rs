//! Makes sure that interrupt priorities affect the order in which interrupt
//! handlers are called.
use r3::{
    hunk::Hunk,
    kernel::{prelude::*, traits, Cfg, InterruptHandler, InterruptLine, Task},
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
    int: [Option<InterruptLine<System>>; 2],
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System>
            + ~const traits::CfgTask
            + ~const traits::CfgInterruptLine,
    {
        Task::build()
            .start(task_body::<System, D>)
            .priority(0)
            .active(true)
            .finish(b);

        #[allow(clippy::len_zero)] // for symmetry
        let int = [
            if D::INTERRUPT_LINES.len() >= 1 && D::INTERRUPT_PRIORITIES.len() >= 1 {
                let int_line = D::INTERRUPT_LINES[0];
                let pri = D::INTERRUPT_PRIORITIES[0];
                InterruptHandler::build()
                    .line(int_line)
                    .start(isr0::<System, D>)
                    .finish(b);
                Some(
                    InterruptLine::build()
                        .line(int_line)
                        .priority(pri)
                        .enabled(true)
                        .finish(b),
                )
            } else {
                None
            },
            if D::INTERRUPT_LINES.len() >= 2 && D::INTERRUPT_PRIORITIES.len() >= 2 {
                let int_line = D::INTERRUPT_LINES[1];
                let pri = D::INTERRUPT_PRIORITIES[1];
                InterruptHandler::build()
                    .line(int_line)
                    .start(isr1::<System, D>)
                    .finish(b);
                Some(
                    InterruptLine::build()
                        .line(int_line)
                        .priority(pri)
                        .enabled(true)
                        .finish(b),
                )
            } else {
                None
            },
        ];

        let seq = Hunk::<_, SeqTracker>::build().finish(b);

        App { int, seq }
    }
}

fn task_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
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

fn isr1<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    log::trace!("isr1");

    D::app().seq.expect_and_replace(2, 3);

    D::success();
}

fn isr0<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    log::trace!("isr0");

    D::app().seq.expect_and_replace(1, 2);

    if D::app().int[1].is_none() {
        log::warn!("Only one interrupt line is defined, skipping the second part of the test");
        D::success();
        return;
    }
}
