//! A task waits for an event bit to be set with timeout.
//!
//! 1. (`seq`: 0 → 1, 0ms) `task1` starts waiting for an event bit to be set.
//! 2. (`seq`: 1 → 2, 0ms) `task0` starts sleeping, which will last for 300
//!    milliseconds.
//! 3. (`seq`: 2 → 3, 200ms) `task1` wakes up, seeing that the wait operation
//!    timed out. `task1` again starts waiting for the event bit to be set.
//! 4. (`seq`: 3 → 4, 300ms) `task0` wakes up and sets the event bit.
//! 5. (`seq`: 4 → 5, 300ms) `task1` wakes up and preempts `task0`, seeing that
//!    the wait operation was successful. Another wait operation will not block
//!    because the event bit is already set.
//! 6. (`seq`: 5 → 6, 300ms) `task1` exits.
//! 7. (`seq`: 6 → 7, 300ms) `task0` starts running.
//!
use constance::{
    hunk::Hunk,
    kernel::{
        cfg::CfgBuilder, EventGroup, EventGroupWaitFlags, QueueOrder, Task,
        WaitEventGroupTimeoutError,
    },
    prelude::*,
    time::Duration,
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    eg: EventGroup<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        Task::build()
            .start(task0_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        Task::build()
            .start(task1_body::<System, D>)
            .priority(1)
            .active(true)
            .finish(b);

        let eg = EventGroup::build().queue_order(QueueOrder::Fifo).finish(b);
        let seq = Hunk::<_, SeqTracker>::build().finish(b);

        App { eg, seq }
    }
}

fn task0_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(1, 2);
    System::sleep(Duration::from_millis(300)).unwrap();
    // `task0` goes into sleep. `task1` wakes up first.
    // `task0` follows:
    D::app().seq.expect_and_replace(3, 4);
    D::app().eg.set(0b1).unwrap();
    // preempted by `task1`, which we just woke up

    // back from `task1`
    D::app().seq.expect_and_replace(6, 7);
    D::success();
}

fn task1_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    assert_eq!(
        // start waiting, switching to `task0`
        D::app().eg.wait_timeout(
            0b1,
            EventGroupWaitFlags::empty(),
            Duration::from_millis(200)
        ),
        // ... the control is returned on timeout
        Err(WaitEventGroupTimeoutError::Timeout),
    );

    D::app().seq.expect_and_replace(2, 3);

    // start waiting. wakes up when `task0` sets the event bit
    D::app()
        .eg
        .wait_timeout(
            0b1,
            EventGroupWaitFlags::empty(),
            Duration::from_millis(200),
        )
        .unwrap();

    D::app().seq.expect_and_replace(4, 5);

    // this doesn't block because the event bit is already set
    D::app()
        .eg
        .wait_timeout(
            0b1,
            EventGroupWaitFlags::empty(),
            Duration::from_millis(200),
        )
        .unwrap();

    D::app().seq.expect_and_replace(5, 6);
}
