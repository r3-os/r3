//! Activates a same-priority task.
use r3::{
    hunk::Hunk,
    kernel::{traits, Cfg, Task},
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
        Task::define()
            .start(task1_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        let task2 = Task::define()
            .start(task2_body::<System, D>)
            .priority(2)
            .finish(b);

        let seq = Hunk::<_, SeqTracker>::define().finish(b);

        App { task2, seq }
    }
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    log::trace!("Good morning, Angel!");
    D::app().task2.activate().unwrap();

    // The task is already active
    assert_eq!(
        D::app().task2.activate(),
        Err(r3::kernel::ActivateTaskError::QueueOverflow)
    );

    D::app().seq.expect_and_replace(1, 2);
}

fn task2_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(2, 3);

    log::trace!("*Rabbit noise*");
    D::success();
}
