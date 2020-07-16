//! Verifies the adjustable range of [`adjust_time`].
//!
//! [`adjust_time`]: constance::kernel::Kernel::adjust_time
use constance::{
    kernel::{AdjustTimeError, Hunk, Task, TIME_USER_HEADROOM},
    prelude::*,
    time::{Duration, Time},
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    task2: Task<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            new! { Task<_>, start = task1_body::<System, D>, priority = 3, active = true };
            let task2 = new! { Task<_>, start = task2_body::<System, D>, priority = 1 };

            let seq = new! { Hunk<_, SeqTracker> };

            App { task2, seq }
        }
    }
}

fn task1_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);
    System::set_time(Time::from_millis(0)).unwrap();
    D::app().task2.activate().unwrap();
    D::app().seq.expect_and_replace(2, 3);

    System::boost_priority().unwrap();

    // `system_time += TIME_USER_HEADROOM + 1300ms`, which should fail because
    // `task2`'s timeout would be late by `300ms`
    log::debug!("system_time += TIME_USER_HEADROOM + 1300ms (should fail)");
    assert_eq!(
        System::adjust_time(TIME_USER_HEADROOM + Duration::from_millis(1300)),
        Err(AdjustTimeError::BadObjectState),
    );

    // `system_time += 500ms`, which should succeed because
    // `task2`'s timeout will not be late
    log::debug!("system_time += 500ms");
    System::adjust_time(Duration::from_millis(500)).unwrap();

    // `system_time += 800ms`, which should succeed because
    // `task2`'s timeout will be only late by `300ms`
    log::debug!("system_time += 800ms");
    System::adjust_time(Duration::from_millis(800)).unwrap();

    // `system_time -= 700ms`, which should succeed because the frontier will be
    // only away by `700ms`
    log::debug!("system_time -= 700ms");
    System::adjust_time(Duration::from_millis(-700)).unwrap();

    // `system_time -= TIME_USER_HEADROOM`, which should fail because the
    // frontier would be away by `700ms + TIME_USER_HEADROOM`
    log::debug!("system_time -= TIME_USER_HEADROOM (should fail)");
    assert_eq!(
        System::adjust_time(Duration::from_millis(-TIME_USER_HEADROOM.as_millis())),
        Err(AdjustTimeError::BadObjectState),
    );

    // `system_time -= TIME_USER_HEADROOM - 900ms`, which should succeed because the frontier will be
    // only away by `TIME_USER_HEADROOM - 200ms`
    log::debug!("system_time -= TIME_USER_HEADROOM - 900ms");
    System::adjust_time(TIME_USER_HEADROOM - Duration::from_millis(900)).unwrap();

    D::app().seq.expect_and_replace(3, 4);

    unsafe { System::unboost_priority().unwrap() };
}

fn task2_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(1, 2);

    // Create a timeout scheduled at 1000ms
    System::sleep(Duration::from_millis(1000)).unwrap();

    D::app().seq.expect_and_replace(4, 5);

    D::success();
}
