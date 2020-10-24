// Disabled test cases are replaced with this module.
use constance::{
    kernel::{cfg::CfgBuilder, StartupHook},
    prelude::*,
};
use core::marker::PhantomData;

use super::Driver;

pub struct App<System> {
    _phantom: PhantomData<System>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        StartupHook::build()
            .start(|_| {
                log::warn!("some crate features are missing, skipping the test");
                D::success();
            })
            .finish(b);

        App {
            _phantom: PhantomData,
        }
    }
}
