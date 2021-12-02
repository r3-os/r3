//! A task waits for a semaphore to be signaled with timeout.
//!
//! 1. (`seq`: 0 → 1, 0ms) `task1` starts waiting for a semaphore to be
//!    signaled.
//! 2. (`seq`: 1 → 2, 0ms) `task0` starts sleeping, which will last for 300
//!    milliseconds.
//! 3. (`seq`: 2 → 3, 200ms) `task1` wakes up, seeing that the wait operation
//!    timed out. `task1` again starts waiting for the semaphore to be signaled.
//! 4. (`seq`: 3 → 4, 300ms) `task0` wakes up and signals the semaphore.
//! 5. (`seq`: 4 → 5, 300ms) `task1` wakes up and preempts `task0`, seeing that
//!    the wait operation was successful.
//! 6. (`seq`: 5 → 6, 300ms) `task1` exits.
//! 7. (`seq`: 6 → 7, 300ms) `task0` starts running.
//!
use r3::{
    hunk::Hunk,
    kernel::{prelude::*, traits, Cfg, Semaphore, Task, WaitSemaphoreTimeoutError},
    time::Duration,
};

use super::Driver;
use crate::utils::SeqTracker;

pub trait SupportedSystem:
    traits::KernelBase + traits::KernelSemaphore + traits::KernelStatic
{
}
impl<T: traits::KernelBase + traits::KernelSemaphore + traits::KernelStatic> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    eg: Semaphore<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System>
            + ~const traits::CfgTask
            + ~const traits::CfgSemaphore,
    {
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

        let eg = Semaphore::build().initial(0).maximum(1).finish(b);
        let seq = Hunk::<_, SeqTracker>::build().finish(b);

        App { eg, seq }
    }
}

fn task0_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    let App { seq, eg } = D::app();

    seq.expect_and_replace(1, 2);
    System::sleep(Duration::from_millis(300)).unwrap();
    // `task0` goes into sleep. `task1` wakes up first.
    // `task0` follows:
    seq.expect_and_replace(3, 4);
    eg.signal(1).unwrap();
    // preempted by `task1`, which we just woke up

    // back from `task1`
    seq.expect_and_replace(6, 7);
    D::success();
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    let App { seq, eg } = D::app();

    seq.expect_and_replace(0, 1);

    assert_eq!(
        // start waiting, switching to `task0`
        eg.wait_one_timeout(Duration::from_millis(200)),
        // ... the control is returned on timeout
        Err(WaitSemaphoreTimeoutError::Timeout),
    );

    seq.expect_and_replace(2, 3);

    // start waiting. wakes up when `task0` signals the semaphore
    eg.wait_one_timeout(Duration::from_millis(200)).unwrap();

    seq.expect_and_replace(4, 5);

    // this doesn't block
    eg.signal(1).unwrap();
    eg.wait_one_timeout(Duration::from_millis(200)).unwrap();

    seq.expect_and_replace(5, 6);
}
