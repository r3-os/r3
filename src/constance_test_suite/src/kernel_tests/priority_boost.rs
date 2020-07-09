//! Activates and deactivates Priority Boost.
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
    assert!(!System::is_priority_boost_active());

    // Activate Priority Boost
    System::boost_priority().unwrap();

    // Can't do it again because it's already acquired
    assert!(System::is_priority_boost_active());
    assert_eq!(
        System::boost_priority(),
        Err(constance::kernel::BoostPriorityError::BadContext),
    );
    assert!(System::is_priority_boost_active());

    // -------------------------------------------------------------------

    // Try acquiring CPU Lock while Priority Boost being active
    System::acquire_cpu_lock().unwrap();
    assert!(System::has_cpu_lock());
    unsafe { System::release_cpu_lock() }.unwrap();

    // Blocking operations are disallowed
    assert_eq!(
        System::park(),
        Err(constance::kernel::ParkError::BadContext),
    );

    // -------------------------------------------------------------------

    // Deactivate Priority Boost
    unsafe { System::unboost_priority() }.unwrap();

    // Can't do it again because it's already deactivated
    assert!(!System::is_priority_boost_active());
    assert_eq!(
        unsafe { System::unboost_priority() },
        Err(constance::kernel::BoostPriorityError::BadContext),
    );
    assert!(!System::is_priority_boost_active());

    // -------------------------------------------------------------------

    // Acquire CPU Lock, and see that Priority Boost doesn't activate in it
    System::acquire_cpu_lock().unwrap();
    assert_eq!(
        System::boost_priority(),
        Err(constance::kernel::BoostPriorityError::BadContext),
    );
    unsafe { System::release_cpu_lock() }.unwrap();

    assert!(!System::is_priority_boost_active());

    D::success();
}
