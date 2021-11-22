//! Checks that Boost Priority is deactivated when a task completes.
use r3::kernel::{traits, Cfg, Task};

use super::Driver;
use crate::utils::conditional::KernelBoostPriorityExt;

pub trait SupportedSystem: traits::KernelBase + KernelBoostPriorityExt {}
impl<T: traits::KernelBase + KernelBoostPriorityExt> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    task2: Task<System>,
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

        App { task2 }
    }
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    // Activate `task2` twice
    D::app().task2.activate().unwrap();
    D::app().task2.activate().unwrap();
    D::success();
}

fn task2_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    // Activate Priority Boost. This should succeed in both runs because
    // it's automatically deactivated on each run.
    if let Some(cap) = System::BOOST_PRIORITY_CAPABILITY {
        System::boost_priority(cap).unwrap();
    }
}
