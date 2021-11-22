//! Signals a semaphore, waking up a task.
use r3::{
    hunk::Hunk,
    kernel::{traits, Cfg, Semaphore, Task},
};

use super::Driver;
use crate::utils::SeqTracker;

pub trait SupportedSystem:
    traits::KernelBase + traits::KernelSemaphore + traits::KernelStatic
{
}
impl<T: traits::KernelBase + traits::KernelSemaphore + traits::KernelStatic> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    sem: Semaphore<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System>
            + ~const traits::CfgTask
            + ~const traits::CfgSemaphore,
    {
        Task::build()
            .start(task1_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        Task::build()
            .start(task2_body::<System, D>)
            .priority(1)
            .active(true)
            .finish(b);
        Task::build()
            .start(task3_body::<System, D>)
            .priority(1)
            .active(true)
            .finish(b);

        let sem = Semaphore::build().initial(0).maximum(2).finish(b);
        let seq = Hunk::<_, SeqTracker>::build().finish(b);

        App { sem, seq }
    }
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(2, 3);

    assert_eq!(D::app().sem.get().unwrap(), 0);
    D::app().sem.signal(2).unwrap(); // unblocks `task2`, `task3`

    D::app().seq.expect_and_replace(5, 6);

    assert_eq!(D::app().sem.get().unwrap(), 0);
    D::app().sem.signal(2).unwrap(); // unblocks `task2`

    assert_eq!(D::app().sem.get().unwrap(), 1);
    D::app().seq.expect_and_replace(7, 8);

    D::success();
}

fn task2_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    D::app().sem.wait_one().unwrap(); // start waiting, switching to `task3`

    D::app().seq.expect_and_replace(3, 4);

    D::app().sem.wait_one().unwrap(); // start waiting, switching to `task1`

    D::app().seq.expect_and_replace(6, 7);
}

fn task3_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(1, 2);

    D::app().sem.wait_one().unwrap(); // start waiting, switching to `task1`

    D::app().seq.expect_and_replace(4, 5);
}
