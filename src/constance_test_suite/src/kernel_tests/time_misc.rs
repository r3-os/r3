//! Validates error codes returned by time-related methods. Also, checks
//! miscellaneous properties of such methods.
use constance::{kernel::Task, prelude::*, time::Time};

use super::Driver;

#[derive(Debug)]
pub struct App<System> {
    task: Task<System>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            let task = new! { Task<_>, start = task_body::<System, D>, priority = 0, active = true };

            App { task }
        }
    }
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let now = System::time().unwrap();
    log::trace!("time = {:?}", now);

    // Because this task is activated at boot, the current time should be
    // very close to zero
    assert_eq!(now.as_secs(), 0);

    // Now change the time
    let now2 = Time::from_millis(114514);
    log::trace!("changing system time to {:?}", now2);
    System::set_time(now2).unwrap();

    // Because we just changed the time to `now2`, the current time should be
    // still very close to `now2`
    let now2_got = System::time().unwrap();
    log::trace!("time = {:?}", now2_got);
    assert_eq!(now2_got.duration_since(now2).unwrap().as_secs(), 0);

    // CPU Lock active
    System::acquire_cpu_lock().unwrap();
    assert_eq!(
        System::time(),
        Err(constance::kernel::TimeError::BadContext)
    );
    assert_eq!(
        System::set_time(now),
        Err(constance::kernel::TimeError::BadContext)
    );
    unsafe { System::release_cpu_lock().unwrap() };

    D::success();
}
