//! Launches multiple tasks, each of which calls `sleep` repeatedly.
use constance::{
    kernel::{cfg::CfgBuilder, EventGroup, EventGroupWaitFlags, Task},
    prelude::*,
    time::{Duration, Time},
};

use super::Driver;

pub struct App<System> {
    done: EventGroup<System>,
}

const TASKS: &[usize] = &[300, 150, 300, 320, 580, 900, 500, 750, 170];

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        let mut i = 0;
        // FIXME: Work-around for `for` being unsupported in `const fn`
        while i < TASKS.len() {
            Task::build()
                .start(task_body::<System, D>)
                .param(i)
                .priority(0)
                .active(true)
                .finish(b);
            i += 1;
        }

        Task::build()
            .start(completion_task_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);

        let done = EventGroup::build().finish(b);

        App { done }
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

    D::app().done.set(1 << i).unwrap();
}

fn completion_task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    // Wait until all tasks run to completion
    D::app()
        .done
        .wait((1 << TASKS.len()) - 1, EventGroupWaitFlags::ALL)
        .unwrap();

    D::success();
}
