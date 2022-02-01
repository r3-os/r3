//! Runs a task at startup.
use core::marker::PhantomData;
use r3::kernel::{traits, Cfg, StaticTask};

use super::Driver;

pub struct App<System> {
    _phantom: PhantomData<System>,
}

impl<System: traits::KernelBase> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System> + ~const traits::CfgTask,
    {
        StaticTask::define()
            .start(task_body::<System, D>)
            .priority(0)
            .active(true)
            .finish(b);

        App {
            _phantom: PhantomData,
        }
    }
}

fn task_body<System: traits::KernelBase, D: Driver<App<System>>>() {
    log::trace!("Good morning, Angel!");
    D::success();
}
