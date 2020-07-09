//! Validates error codes returned by time-related methods. Also, checks
//! miscellaneous properties of such methods.
use constance::{
    kernel::Task,
    prelude::*,
    time::{Duration, Time},
};

use super::Driver;

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

    // System time should wrap around
    let now3 = Time::from_micros(0xfffffffffffe0000);
    log::trace!("changing system time to {:?}", now3);
    System::set_time(now3).unwrap();

    let d = Duration::from_micros(0x40000);
    log::trace!("sleeping for {:?}", d);
    System::sleep(d).unwrap();

    // TODO: `impl Add<Duration> for Time`
    let now4 = Time::from_micros(now3.as_micros().wrapping_add(d.as_micros() as _));
    let now4_got = System::time().unwrap();
    log::trace!("time = {:?} (expected >= {:?})", now4_got, now4);
    assert!(now4_got.as_micros() >= now4.as_micros());

    D::success();
}
