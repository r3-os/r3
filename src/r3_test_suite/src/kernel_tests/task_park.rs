//! Sequence the execution of tasks using the parking mechanism.
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
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System> + ~const traits::CfgTask,
    {
        StaticTask::define()
            .start(task1_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        let task2 = StaticTask::define()
            .start(task2_body::<System, D>)
            .priority(1)
            .active(true)
            .finish(b);

        let seq = Hunk::<_, SeqTracker>::define().finish(b);

        App { task2, seq }
    }
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>() {
    D::app().seq.expect_and_replace(1, 2);

    D::app().task2.unpark_exact().unwrap();

    D::app().seq.expect_and_replace(3, 4);

    D::app().task2.interrupt().unwrap();
}

fn task2_body<System: SupportedSystem, D: Driver<App<System>>>() {
    D::app().seq.expect_and_replace(0, 1);

    System::park().unwrap(); // blocks, switching to `task1`

    D::app().seq.expect_and_replace(2, 3);

    assert_eq!(
        // blocks, switching to `task1`
        System::park(),
        Err(r3::kernel::ParkError::Interrupted)
    );

    D::app().seq.expect_and_replace(4, 5);

    // Give a park token to itself
    D::app().task2.unpark_exact().unwrap();
    // `park` doesn't block if the task already has a token
    System::park().unwrap();

    D::app().task2.unpark_exact().unwrap();
    assert_eq!(
        D::app().task2.unpark_exact(),
        Err(r3::kernel::UnparkExactError::QueueOverflow)
    );

    D::success();
}
