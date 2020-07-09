//! Launches multiple tasks, each of which calls `sleep` repeatedly.
use constance::{
    kernel::{Hunk, Task},
    prelude::*,
    time::{Duration, Time},
};
use core::sync::atomic::{AtomicUsize, Ordering};

use super::Driver;

pub struct App<System> {
    counter: Hunk<System, AtomicUsize>,
}

const TASKS: &[usize] = &[300, 150, 300, 320, 580, 900, 500, 750, 170];

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            let mut i = 0;
            // FIXME: Work-around for `for` being unsupported in `const fn`
            while i < TASKS.len() {
                new! { Task<_>,
                    start = task_body::<System, D>, param = i, priority = 0, active = true };
                i += 1;
            }

            let counter = new! { Hunk<_, AtomicUsize> };

            App { counter }
        }
    }
}

fn task_body<System: Kernel, D: Driver<App<System>>>(i: usize) {
    let delay = Duration::from_millis(TASKS[i] as _);

    loop {
        let now = System::time().unwrap();
        log::trace!("[{}] time = {:?}", i, now);

        if now.as_secs() >= 2 {
            break;
        }

        System::sleep(delay).unwrap();

        let now2 = Time::from_micros(now.as_micros().wrapping_add(delay.as_micros() as _));
        let now2_got = System::time().unwrap();
        log::trace!("[{}] time = {:?} (expected = {:?})", i, now2_got, now2);

        // `now2 <= now2_got < now2 + timing_error`
        let delta = now2_got.duration_since(now2);
        assert!(!delta.unwrap().is_negative());
        assert!(delta.unwrap().as_millis() < 100);
    }

    if D::app().counter.fetch_add(1, Ordering::Relaxed) == TASKS.len() - 1 {
        D::success();
    }
}
