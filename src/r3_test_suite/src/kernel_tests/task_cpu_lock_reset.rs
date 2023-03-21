//! Checks that CPU Lock is released when a task completes.
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
        C: ~const traits::CfgTask<System = System>,
    {
        StaticTask::define()
            .start(task1_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        let task2 = StaticTask::define()
            .start(task2_body::<System>)
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

fn task2_body<System: SupportedSystem>() {
    // Acquire CPU Lock This should succeed in both runs because
    // it's automatically released on each run.
    System::acquire_cpu_lock().unwrap();
}
