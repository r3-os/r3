//! Interrupts a task waiting for an event bit to be set.
//!
//! 1. (`seq`: 0 → 1) `task1` starts waiting for an event bit to be set.
//! 2. (`seq`: 1 → 2) `task0` starts running and interrupts `task0`.
//! 3. (`seq`: 2 → 3) `task1` starts waiting for an event bit to be set, this
//!    time with a timeout.
//! 4. (`seq`: 3 → 4) `task0` interrupts `task0`.
//! 5. (`seq`: 4 → 5) `task1` completes.
//!
use r3::{
    hunk::Hunk,
    kernel::{
        prelude::*, traits, Cfg, EventGroupWaitFlags, QueueOrder, StaticEventGroup, StaticTask,
        WaitEventGroupError, WaitEventGroupTimeoutError,
    },
    time::Duration,
};

use super::Driver;
use crate::utils::SeqTracker;

pub trait SupportedSystem:
    traits::KernelBase + traits::KernelEventGroup + traits::KernelStatic
{
}
impl<T: traits::KernelBase + traits::KernelEventGroup + traits::KernelStatic> SupportedSystem
    for T
{
}

pub struct App<System: SupportedSystem> {
    eg: StaticEventGroup<System>,
    task1: StaticTask<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgTask<System = System> + ~const traits::CfgEventGroup,
    {
        StaticTask::define()
            .start(task0_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        let task1 = StaticTask::define()
            .start(task1_body::<System, D>)
            .priority(1)
            .active(true)
            .finish(b);

        let eg = StaticEventGroup::define()
            .queue_order(QueueOrder::Fifo)
            .finish(b);
        let seq = Hunk::<_, SeqTracker>::define().finish(b);

        App { eg, task1, seq }
    }
}

fn task0_body<System: SupportedSystem, D: Driver<App<System>>>() {
    D::app().seq.expect_and_replace(1, 2);
    D::app().task1.interrupt().unwrap();
    D::app().seq.expect_and_replace(3, 4);
    D::app().task1.interrupt().unwrap();
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>() {
    D::app().seq.expect_and_replace(0, 1);

    assert_eq!(
        // start waiting, switching to `task0`
        D::app().eg.wait(0b1, EventGroupWaitFlags::CLEAR),
        // ... the control is returned when `task0` interrupts `task1`
        Err(WaitEventGroupError::Interrupted),
    );

    D::app().seq.expect_and_replace(2, 3);

    assert_eq!(
        // start waiting, switching to `task0`
        D::app()
            .eg
            .wait_timeout(0b1, EventGroupWaitFlags::CLEAR, Duration::from_millis(100)),
        // ... the control is returned when `task0` interrupts `task1`
        Err(WaitEventGroupTimeoutError::Interrupted),
    );

    D::app().seq.expect_and_replace(4, 5);

    D::success();
}
