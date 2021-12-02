//! Verifies the adjustable range of [`adjust_time`].
//!
//! [`adjust_time`]: r3::kernel::Kernel::adjust_time
use r3::{
    hunk::Hunk,
    kernel::{prelude::*, traits, AdjustTimeError, Cfg, Task},
    time::Duration,
};

use super::Driver;
use crate::utils::{conditional::KernelTimeExt, SeqTracker};

pub trait SupportedSystem:
    traits::KernelBase
    + traits::KernelAdjustTime
    + traits::KernelBoostPriority
    + traits::KernelStatic
    + KernelTimeExt
{
}
impl<
        T: traits::KernelBase
            + traits::KernelAdjustTime
            + traits::KernelBoostPriority
            + traits::KernelStatic
            + KernelTimeExt,
    > SupportedSystem for T
{
}

pub struct App<System: SupportedSystem> {
    task2: Task<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System> + ~const traits::CfgTask,
    {
        Task::build()
            .start(task1_body::<System, D>)
            .priority(3)
            .active(true)
            .finish(b);
        let task2 = Task::build()
            .start(task2_body::<System, D>)
            .priority(1)
            .finish(b);

        let seq = Hunk::<_, SeqTracker>::build().finish(b);

        App { task2, seq }
    }
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);
    D::app().task2.activate().unwrap();
    D::app().seq.expect_and_replace(2, 3);

    System::boost_priority().unwrap();

    let time_user_headroom = System::time_user_headroom();

    // `time_user_headroom` must be at least one second
    assert!(time_user_headroom >= Duration::from_secs(1));

    if D::TIME_USER_HEADROOM_IS_EXACT {
        // `system_time += time_user_headroom + 1300ms`, which should fail because
        // `task2`'s timeout would be late by `300ms`
        log::debug!("system_time += time_user_headroom + 1300ms (should fail)");
        assert_eq!(
            System::adjust_time(time_user_headroom + Duration::from_millis(1300)),
            Err(AdjustTimeError::BadObjectState),
        );
    }

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

    if D::TIME_USER_HEADROOM_IS_EXACT {
        // `system_time -= time_user_headroom`, which should fail because the
        // frontier would be away by `700ms + time_user_headroom`
        log::debug!("system_time -= time_user_headroom (should fail)");
        assert_eq!(
            System::adjust_time(Duration::from_millis(-time_user_headroom.as_millis())),
            Err(AdjustTimeError::BadObjectState),
        );
    }

    // `system_time -= time_user_headroom - 900ms`, which should succeed because the frontier will be
    // only away by `time_user_headroom - 200ms`
    log::debug!("system_time -= time_user_headroom - 900ms");
    System::adjust_time(time_user_headroom - Duration::from_millis(900)).unwrap();

    D::app().seq.expect_and_replace(3, 4);

    unsafe { System::unboost_priority().unwrap() };
}

fn task2_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(1, 2);

    // Create a timeout scheduled at 1000ms
    System::sleep_ms(1000);

    D::app().seq.expect_and_replace(4, 5);

    D::success();
}
