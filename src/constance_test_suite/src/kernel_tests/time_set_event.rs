//! Verifies that `set_time` maintains the relative arrival times of
//! timeouts.
//!
//! [`set_time`]: constance::kernel::Kernel::set_time
//!
//! 1. (`seq`: 0 → 1, 0ms) `task1` activates `task2` and `task3`.
//! 2. (`seq`: 1 → 2, 0ms) `task2` starts sleeping, expecting to be woken up
//!    at system time 300ms.
//! 3. (`seq`: 2 → 3, 0ms) `task3` starts sleeping, expecting to be woken up
//!    at system time 100ms.
//! 4. (`seq`: 3 → 4, 0ms) `task1` changes the system time to 300ms using
//!    `set_time`.
//! 6. (`seq`: 4 → 5, 300ms) `task1` exits.
//! 5. (`seq`: 5 → 6, 400ms) `task3` wakes up.
//! 7. (`seq`: 6 → 7, 600ms) `tsak2` wakes up.
//!
use constance::{
    kernel::{Hunk, Task},
    prelude::*,
    time::{Duration, Time},
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    task2: Task<System>,
    task3: Task<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            new! { Task<_>, start = task1_body::<System, D>, priority = 3, active = true };
            let task2 = new! { Task<_>, start = task2_body::<System, D>, priority = 1 };
            let task3 = new! { Task<_>, start = task3_body::<System, D>, priority = 2 };

            let seq = new! { Hunk<_, SeqTracker> };

            App { task2, task3, seq }
        }
    }
}

fn task1_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);
    System::set_time(Time::from_millis(0)).unwrap();
    D::app().task2.activate().unwrap();
    D::app().task3.activate().unwrap();
    D::app().seq.expect_and_replace(3, 4);

    // Adjust the system time while `task2` and `task3` are sleeping.
    System::set_time(Time::from_millis(300)).unwrap();

    D::app().seq.expect_and_replace(4, 5);
}

fn task2_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(1, 2);

    // Start sleeping at system time 0ms
    System::sleep(Duration::from_millis(300)).unwrap();

    D::app().seq.expect_and_replace(6, 7);

    // Sleeping should conclude at system time 600ms
    let now = Time::from_millis(600);
    let now_got = System::time().unwrap();
    log::trace!("time = {:?} (expected = {:?})", now_got, now);
    assert!(
        now_got
            .duration_since(now)
            .unwrap()
            .checked_abs()
            .unwrap()
            .as_millis()
            < 50
    );

    D::success();
}

fn task3_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(2, 3);

    // Start sleeping at system time 0ms
    System::sleep(Duration::from_millis(100)).unwrap();

    D::app().seq.expect_and_replace(5, 6);

    // Sleeping should conclude at system time 400ms
    let now = Time::from_millis(400);
    let now_got = System::time().unwrap();
    log::trace!("time = {:?} (expected = {:?})", now_got, now);
    assert!(
        now_got
            .duration_since(now)
            .unwrap()
            .checked_abs()
            .unwrap()
            .as_millis()
            < 50
    );
}
