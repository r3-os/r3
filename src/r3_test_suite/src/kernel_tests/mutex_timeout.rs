//! Tasks wait for a mutex to be unlocked with timeout.
//!
//! 1. (`seq`: 0 → 1, 0ms) `task0` locks a mutex and activates `task1`.
//! 2. (`seq`: 1 → 2, 0ms) `task1` starts waiting for the mutex to be
//!    unlocked.
//! 3. (`seq`: 2 → 3, 0ms) `task0` starts sleeping, which will last for 300
//!    milliseconds.
//! 4. (`seq`: 3 → 4, 200ms) `task1` wakes up, seeing that the wait operation
//!    timed out. `task1` again starts waiting for the mutex to be unlocked.
//! 5. (`seq`: 4 → 5, 300ms) `task0` wakes up and unlocks the mutex.
//! 6. (`seq`: 5 → 6, 300ms) `task1` wakes up and preempts `task0`, seeing that
//!    the wait operation was successful.
//! 7. (`seq`: 6 → 7, 300ms) `task1` starts sleeping, which will last for 200
//!    milliseconds.
//! 8. (`seq`: 7 → 8, 300ms) `task0` starts running and waiting for the mutex to
//!    be unlocked.
//! 7. (`seq`: 8 → 9, 500ms) `task1` exits, which implicitly abandons the mutex.
//! 8. (`seq`: 9 → 10, 500ms) `task0` gets unblocked, seeing that the mutex has
//!    been abandoned by `task1`.
//!
use r3::{
    hunk::Hunk,
    kernel::{prelude::*, traits, Cfg, LockMutexTimeoutError, StaticMutex, StaticTask},
    time::Duration,
};

use super::Driver;
use crate::utils::SeqTracker;

pub trait SupportedSystem: traits::KernelBase + traits::KernelMutex + traits::KernelStatic {}
impl<T: traits::KernelBase + traits::KernelMutex + traits::KernelStatic> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    eg: StaticMutex<System>,
    task1: StaticTask<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgTask<System = System> + ~const traits::CfgMutex,
    {
        StaticTask::define()
            .start(task0_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        let task1 = StaticTask::define()
            .start(task1_body::<System, D>)
            .priority(1)
            .active(false)
            .finish(b);

        let eg = StaticMutex::define().finish(b);
        let seq = Hunk::<_, SeqTracker>::define().finish(b);

        App { task1, eg, seq }
    }
}

fn task0_body<System: SupportedSystem, D: Driver<App<System>>>() {
    let App { seq, eg, task1 } = D::app();

    seq.expect_and_replace(0, 1);
    eg.lock().unwrap();
    task1.activate().unwrap();

    seq.expect_and_replace(2, 3);
    System::sleep(Duration::from_millis(300)).unwrap();
    // `task0` goes into sleep. `task1` wakes up first.
    // `task0` follows:
    seq.expect_and_replace(4, 5);
    eg.unlock().unwrap();
    // preempted by `task1`, which we just woke up

    // back from `task1`
    seq.expect_and_replace(7, 8);
    assert_eq!(
        eg.lock_timeout(Duration::from_millis(500)),
        Err(LockMutexTimeoutError::Abandoned)
    );
    seq.expect_and_replace(9, 10);
    D::success();
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>() {
    let App { seq, eg, .. } = D::app();

    seq.expect_and_replace(1, 2);

    assert_eq!(
        // start waiting, switching to `task0`
        eg.lock_timeout(Duration::from_millis(200)),
        // ... the control is returned on timeout
        Err(LockMutexTimeoutError::Timeout),
    );

    seq.expect_and_replace(3, 4);

    // start waiting. wakes up when `task0` unlocks the mutex
    eg.lock_timeout(Duration::from_millis(200)).unwrap();

    seq.expect_and_replace(5, 6);

    // this doesn't block
    eg.unlock().unwrap();
    eg.lock_timeout(Duration::from_millis(200)).unwrap();

    seq.expect_and_replace(6, 7);
    System::sleep(Duration::from_millis(200)).unwrap();
    seq.expect_and_replace(8, 9);
}
