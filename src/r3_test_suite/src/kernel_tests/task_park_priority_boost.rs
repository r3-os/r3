//! Sequence the execution of tasks using the parking mechanism. Priority Boost
//! is used to temporarily suppress preemption.
use r3::{
    hunk::Hunk,
    kernel::{prelude::*, traits, Cfg, Task},
};

use super::Driver;
use crate::utils::SeqTracker;

pub trait SupportedSystem:
    traits::KernelBase + traits::KernelBoostPriority + traits::KernelStatic
{
}
impl<T: traits::KernelBase + traits::KernelBoostPriority + traits::KernelStatic> SupportedSystem
    for T
{
}

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
            .priority(1)
            .active(true)
            .finish(b);

        let seq = Hunk::<_, SeqTracker>::define().finish(b);

        App { task2, seq }
    }
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(1, 2);

    // Activate Boost Priority
    System::boost_priority().unwrap();

    // Boost Priority is active, so `task2` can't preempt yet even though it
    // has a higher priority
    D::app().task2.unpark_exact().unwrap();

    D::app().seq.expect_and_replace(2, 3);

    // Deactive Boost Priority. Now `task2` can preempt and run to completion
    unsafe { System::unboost_priority() }.unwrap();

    D::app().seq.expect_and_replace(4, 5);

    D::success();
}

fn task2_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    System::park().unwrap(); // blocks, switching to `task1`

    D::app().seq.expect_and_replace(3, 4);

    // `task2` is done, now go back to `task1`
}
