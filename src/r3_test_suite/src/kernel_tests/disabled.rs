// Disabled test cases are replaced with this module.
use core::marker::PhantomData;
use r3::kernel::{traits, Cfg, StartupHook};

use super::Driver;

pub struct App<System> {
    _phantom: PhantomData<System>,
}

impl<System: traits::KernelBase> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgTask<System = System>,
    {
        StartupHook::define()
            .start(|| {
                log::warn!("some crate features are missing, skipping the test");
                D::success();
            })
            .finish(b);

        App {
            _phantom: PhantomData,
        }
    }
}
