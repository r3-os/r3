//! Checks that a park token is reset when a task is activated.
use r3::kernel::{prelude::*, traits, Cfg, StaticTask};

use super::Driver;

pub trait SupportedSystem: traits::KernelBase {}
impl<T: traits::KernelBase> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    task2: StaticTask<System>,
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
            .finish(b);

        App { task2 }
    }
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>() {
    // Activate `task2` twice
    D::app().task2.activate().unwrap();
    D::app().task2.activate().unwrap();
    D::success();
}

fn task2_body<System: SupportedSystem, D: Driver<App<System>>>() {
    // Give a park token to itself. This should succeed in both runs because
    // the park token is reset on each run.
    D::app().task2.unpark_exact().unwrap();
}
