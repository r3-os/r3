//! Runs a task at startup.
use constance::{
    kernel::{cfg::CfgBuilder, Task},
    prelude::*,
};
use core::marker::PhantomData;

use super::Driver;

pub struct App<System> {
    _phantom: PhantomData<System>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        Task::build()
            .start(task_body::<System, D>)
            .priority(0)
            .active(true)
            .finish(b);

        App {
            _phantom: PhantomData,
        }
    }
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    log::trace!("Good morning, Angel!");
    D::success();
}
