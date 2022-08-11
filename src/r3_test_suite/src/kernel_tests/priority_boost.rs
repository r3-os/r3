//! Activates and deactivates Priority Boost.
use core::marker::PhantomData;
use r3::kernel::{prelude::*, traits, Cfg, StaticTask};

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
        C: ~const traits::CfgTask<System = System>,
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

#[cfg(feature = "priority_boost")]
fn task_body<System: SupportedSystem, D: Driver<App<System>>>() {
    assert!(!System::is_priority_boost_active());

    // Activate Priority Boost
    System::boost_priority().unwrap();

    // Can't do it again because it's already acquired
    assert!(System::is_priority_boost_active());
    assert_eq!(
        System::boost_priority(),
        Err(r3::kernel::BoostPriorityError::BadContext),
    );
    assert!(System::is_priority_boost_active());

    // -------------------------------------------------------------------

    // Try acquiring CPU Lock while Priority Boost being active
    System::acquire_cpu_lock().unwrap();
    assert!(System::has_cpu_lock());
    unsafe { System::release_cpu_lock() }.unwrap();

    // Blocking operations are disallowed
    assert_eq!(System::park(), Err(r3::kernel::ParkError::BadContext),);

    // -------------------------------------------------------------------

    // Deactivate Priority Boost
    unsafe { System::unboost_priority() }.unwrap();

    // Can't do it again because it's already deactivated
    assert!(!System::is_priority_boost_active());
    assert_eq!(
        unsafe { System::unboost_priority() },
        Err(r3::kernel::BoostPriorityError::BadContext),
    );
    assert!(!System::is_priority_boost_active());

    // -------------------------------------------------------------------

    // Acquire CPU Lock, and see that Priority Boost doesn't activate in it
    System::acquire_cpu_lock().unwrap();
    assert_eq!(
        System::boost_priority(),
        Err(r3::kernel::BoostPriorityError::BadContext),
    );
    unsafe { System::release_cpu_lock() }.unwrap();

    assert!(!System::is_priority_boost_active());

    D::success();
}

#[cfg(not(feature = "priority_boost"))]
fn task_body<System: SupportedSystem, D: Driver<App<System>>>() {
    // Priority Boost is always inactive when it's statically disabled
    assert!(!System::is_priority_boost_active());

    // Can't deactivate Priority Boost because it's already deactivated
    assert_eq!(
        unsafe { System::unboost_priority() },
        Err(r3::kernel::BoostPriorityError::BadContext),
    );

    D::success();
}
