//! Runs a task at startup.
use constance::{kernel::Task, prelude::*};
use core::marker::PhantomData;

use super::Driver;

pub struct App<System> {
    _phantom: PhantomData<System>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            new! { Task<_>, start = task_body::<System, D>, priority = 0, active = true };

            App {
                _phantom: PhantomData,
            }
        }
    }
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    log::trace!("Good morning, Angel!");
    D::success();
}
