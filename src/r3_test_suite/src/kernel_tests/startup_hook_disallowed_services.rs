//! Checks the return codes of disallowed system calls made in a boot context.
use core::marker::PhantomData;
use r3::kernel::{self, prelude::*, traits, Cfg, StartupHook};

use super::Driver;

#[cfg(feature = "priority_boost")]
pub trait SupportedSystem: traits::KernelBase + traits::KernelBoostPriority {}
#[cfg(feature = "priority_boost")]
impl<T: traits::KernelBase + traits::KernelBoostPriority> SupportedSystem for T {}

#[cfg(not(feature = "priority_boost"))]
pub trait SupportedSystem: traits::KernelBase {}
#[cfg(not(feature = "priority_boost"))]
impl<T: traits::KernelBase> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    _phantom: PhantomData<System>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System> + ~const traits::CfgTask,
    {
        StartupHook::build().start(hook::<System, D>).finish(b);

        App {
            _phantom: PhantomData,
        }
    }
}

fn hook<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    assert!(System::has_cpu_lock());

    // Disallowed in a non-task context
    #[cfg(feature = "priority_boost")]
    assert_eq!(
        System::boost_priority(),
        Err(kernel::BoostPriorityError::BadContext),
    );
    assert_eq!(
        unsafe { System::exit_task() },
        Err(kernel::ExitTaskError::BadContext),
    );

    // Blocking system services
    assert_eq!(System::park(), Err(kernel::ParkError::BadContext));

    D::success();
}
