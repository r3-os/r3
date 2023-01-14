//! Asserts that tasks in the same ready queue are processed in a FIFO order.
use r3::{
    hunk::Hunk,
    kernel::{prelude::*, traits, Cfg, StaticTask},
};

use super::Driver;
use crate::utils::SeqTracker;

pub trait SupportedSystem: traits::KernelBase + traits::KernelStatic {}
impl<T: traits::KernelBase + traits::KernelStatic> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    task2: StaticTask<System>,
    task3: StaticTask<System>,
    task4: StaticTask<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgTask<System = System>,
    {
        StaticTask::define()
            .start(task1_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        let task2 = StaticTask::define()
            .start((2, task2_body::<System, D>))
            .priority(2)
            .finish(b);
        let task3 = StaticTask::define()
            .start((3, task2_body::<System, D>))
            .priority(2)
            .finish(b);
        let task4 = StaticTask::define()
            .start((4, task2_body::<System, D>))
            .priority(2)
            .finish(b);

        let seq = Hunk::<_, SeqTracker>::define().finish(b);

        App {
            task2,
            task3,
            task4,
            seq,
        }
    }
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>() {
    D::app().seq.expect_and_replace(0, 1);

    log::trace!("Good morning, Angel!");
    D::app().task2.activate().unwrap();
    D::app().task3.activate().unwrap();
    D::app().task4.activate().unwrap();

    D::app().seq.expect_and_replace(1, 2);
}

fn task2_body<System: SupportedSystem, D: Driver<App<System>>>(i: usize) {
    D::app().seq.expect_and_replace(i, i + 1);
    log::trace!("*Rabbit noise {i}*");
    if i == 4 {
        D::success();
    }
}
