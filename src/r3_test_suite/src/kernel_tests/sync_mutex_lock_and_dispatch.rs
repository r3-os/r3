//! Checks miscellaneous properties of [`r3::sync::StaticMutex`].
use assert_matches::assert_matches;
use r3::{
    hunk::Hunk,
    kernel::{prelude::*, traits, Cfg, StaticTask},
    sync::mutex::{self, StaticMutex},
};

use super::Driver;
use crate::utils::SeqTracker;

pub trait SupportedSystem: traits::KernelBase + traits::KernelMutex + traits::KernelStatic {}
impl<T: traits::KernelBase + traits::KernelMutex + traits::KernelStatic> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    task2: StaticTask<System>,
    mutex: StaticMutex<System, u32>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgTask<System = System> + ~const traits::CfgMutex,
    {
        StaticTask::define()
            .start(task1_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        let task2 = StaticTask::define()
            .start(task2_body::<System, D>)
            .priority(1)
            .active(false)
            .finish(b);

        let mutex = StaticMutex::define().finish(b);

        let seq = Hunk::<_, SeqTracker>::define().finish(b);

        App { task2, mutex, seq }
    }
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>() {
    let app = D::app();

    app.seq.expect_and_replace(0, 1);

    {
        let mut lock = app.mutex.lock().unwrap();
        app.task2.activate().unwrap(); // giving the control to `task2`

        // back from `task2`, which is being blocked...
        app.seq.expect_and_replace(2, 3);
        *lock = 42;

        // release the lock and let `task2` continue. the control will return to
        // here when `task2` completes
    }

    app.seq.expect_and_replace(5, 6);

    assert_eq!(*app.mutex.lock().unwrap(), 56);

    D::success();
}

fn task2_body<System: SupportedSystem, D: Driver<App<System>>>() {
    let app = D::app();

    app.seq.expect_and_replace(1, 2);

    // returns `WouldBlock` because `task1` has lock
    assert_matches!(app.mutex.try_lock(), Err(mutex::TryLockError::WouldBlock));

    {
        let mut lock = app.mutex.lock().unwrap(); // blocks because `task1` has lock

        // preempts `task1` when it releases the lock
        app.seq.expect_and_replace(3, 4);
        assert_eq!(*lock, 42);
        *lock = 56;
    }

    app.seq.expect_and_replace(4, 5);
}
