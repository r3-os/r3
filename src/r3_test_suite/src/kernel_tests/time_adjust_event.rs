//! Verifies that [`adjust_time`] maintains the absolute arrival times of
//! timeouts.
//!
//! [`adjust_time`]: r3::kernel::Kernel::adjust_time
//!
//! 1. (`seq`: 0 → 1, 0ms) `task1` activates `task2` and `task3`.
//! 2. (`seq`: 1 → 2, 0ms) `task2` starts sleeping, expecting to be woken up
//!    at system time 600ms.
//! 3. (`seq`: 2 → 3, 0ms) `task3` starts sleeping, expecting to be woken up
//!    at system time 100ms.
//! 4. (`seq`: 3 → 4, 0ms) `task1` changes the system time to 300ms using
//!    `adjust_time`.
//! 5. (`seq`: 4 → 5, 300ms) `task3` wakes up, finding it's late by 200ms.
//! 6. (`seq`: 5 → 6, 600ms) `tsak2` wakes up.
//!
use r3::{
    hunk::Hunk,
    kernel::{prelude::*, traits, Cfg, StaticTask},
    time::Duration,
};

use super::Driver;
use crate::utils::{conditional::KernelTimeExt, SeqTracker};

pub trait SupportedSystem:
    traits::KernelBase + traits::KernelAdjustTime + traits::KernelStatic + KernelTimeExt
{
}
impl<T: traits::KernelBase + traits::KernelAdjustTime + traits::KernelStatic + KernelTimeExt>
    SupportedSystem for T
{
}

pub struct App<System: SupportedSystem> {
    task2: StaticTask<System>,
    task3: StaticTask<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgTask<System = System>,
    {
        StaticTask::define()
            .start(task1_body::<System, D>)
            .priority(3)
            .active(true)
            .finish(b);
        let task2 = StaticTask::define()
            .start(task2_body::<System, D>)
            .priority(1)
            .finish(b);
        let task3 = StaticTask::define()
            .start(task3_body::<System, D>)
            .priority(2)
            .finish(b);

        let seq = Hunk::<_, SeqTracker>::define().finish(b);

        App { task2, task3, seq }
    }
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>() {
    D::app().seq.expect_and_replace(0, 1);
    System::set_time_ms(0);
    D::app().task2.activate().unwrap();
    D::app().task3.activate().unwrap();
    D::app().seq.expect_and_replace(3, 4);

    // Adjust the system time while `task2` and `task3` are sleeping.
    System::adjust_time(Duration::from_millis(300)).unwrap();
    // This will cause `task3` to wake up very soon.
    // (It's unspecified whether it happens before or after
    // `adjust_time` returns.)
}

fn task2_body<System: SupportedSystem, D: Driver<App<System>>>() {
    D::app().seq.expect_and_replace(1, 2);

    // Start sleeping at system time 0ms
    System::sleep_ms(600);

    D::app().seq.expect_and_replace(5, 6);

    // Sleeping should conclude at system time 600ms
    System::assert_time_ms_range(600..700);

    D::success();
}

fn task3_body<System: SupportedSystem, D: Driver<App<System>>>() {
    D::app().seq.expect_and_replace(2, 3);

    // Start sleeping at system time 0ms
    System::sleep_ms(100);

    D::app().seq.expect_and_replace(4, 5);

    // Sleeping should conclude at system time 300ms (late by 200ms)
    // because it jumped to 300ms
    System::assert_time_ms_range(300..400);
}
