//! Activates a higher-priority task.
use r3::{
    hunk::Hunk,
    kernel::{prelude::*, traits, Cfg, Task},
};

use super::Driver;
use crate::utils::SeqTracker;

pub trait SupportedSystem: traits::KernelBase + traits::KernelStatic {}
impl<T: traits::KernelBase + traits::KernelStatic> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    task2: Task<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System> + ~const traits::CfgTask,
    {
        Task::build()
            .start(task1_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        let task2 = Task::build()
            .start(task2_body::<System, D>)
            .priority(1)
            .finish(b);

        let seq = Hunk::<_, SeqTracker>::build().finish(b);

        App { task2, seq }
    }
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    log::trace!("Good morning, Angel!");
    D::app().task2.activate().unwrap();

    D::app().seq.expect_and_replace(2, 3);
    D::success();
}

fn task2_body<System: SupportedSystem, D: Driver<App<System>>>(param: usize) {
    D::app().seq.expect_and_replace(1, 2);

    log::trace!("*Rabbit noise*");

    if param == 0 {
        // Safety: There's nothing on the stack unsafe to `forget`
        unsafe { System::exit_task().unwrap() };
    }

    unreachable!();
}
