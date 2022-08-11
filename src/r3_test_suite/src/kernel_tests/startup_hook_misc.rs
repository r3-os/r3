//! Checks miscellaneous properties of `StartupHook`.
use core::marker::PhantomData;
use r3::kernel::{prelude::*, traits, Cfg, StartupHook};

use super::Driver;

pub trait SupportedSystem: traits::KernelBase + traits::KernelStatic {}
impl<T: traits::KernelBase + traits::KernelStatic> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    _phantom: PhantomData<System>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgTask<System = System>,
    {
        StartupHook::define().start(hook::<System, D>).finish(b);

        App {
            _phantom: PhantomData,
        }
    }
}

fn hook<System: SupportedSystem, D: Driver<App<System>>>() {
    log::trace!("The startup hook is running");

    // Context query
    assert!(!System::is_task_context());
    assert!(!System::is_interrupt_context());
    assert!(!System::is_boot_complete());

    D::success();
}
