//! Launches multiple tasks, each of which calls `sleep` repeatedly.
use r3::{
    kernel::{prelude::*, traits, Cfg, EventGroupWaitFlags, StaticEventGroup, StaticTask},
    time::{Duration, Time},
};

use super::Driver;

pub trait SupportedSystem:
    traits::KernelBase + traits::KernelTime + traits::KernelEventGroup
{
}
impl<T: traits::KernelBase + traits::KernelTime + traits::KernelEventGroup> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    done: StaticEventGroup<System>,
}

const TASKS: &[usize] = &[300, 150, 300, 750, 170];

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System>
            + ~const traits::CfgTask
            + ~const traits::CfgEventGroup,
    {
        let mut i = 0;
        // `for` is unusable in `const fn` [ref:const_for]
        while i < TASKS.len() {
            StaticTask::define()
                .start((i, task_body::<System, D>))
                .priority(0)
                .active(true)
                .finish(b);
            i += 1;
        }

        StaticTask::define()
            .start(completion_task_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);

        let done = StaticEventGroup::define().finish(b);

        App { done }
    }
}

fn task_body<System: SupportedSystem, D: Driver<App<System>>>(i: usize) {
    let delay = Duration::from_millis(TASKS[i] as _);

    for i in 0.. {
        let now = System::time().unwrap();
        log::trace!("[{}] time = {:?}", i, now);

        if now.as_secs() >= 2 {
            break;
        }

        let delay = if i == 2 {
            // Exponentially increase the interval, up to 500ms. Small delays
            // exercise rarely-reached code paths in the kernel.
            Duration::from_micros(1 << (i / 2).min(19))
        } else {
            delay
        };

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

fn completion_task_body<System: SupportedSystem, D: Driver<App<System>>>() {
    // Wait until all tasks run to completion
    D::app()
        .done
        .wait((1 << TASKS.len()) - 1, EventGroupWaitFlags::ALL)
        .unwrap();

    D::success();
}
