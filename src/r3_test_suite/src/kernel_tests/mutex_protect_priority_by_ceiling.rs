//! A text-book example where a mutex adhereing to the priority ceiling protocol
//! successfully prevents unbounded priority inversion.
use r3::{
    hunk::Hunk,
    kernel::{prelude::*, traits, Cfg, MutexProtocol, StaticMutex, StaticTask},
    time::Duration,
};

use super::Driver;
use crate::utils::SeqTracker;

pub trait SupportedSystem: traits::KernelBase + traits::KernelMutex + traits::KernelStatic {}
impl<T: traits::KernelBase + traits::KernelMutex + traits::KernelStatic> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    task0: StaticTask<System>,
    task1: StaticTask<System>,
    task2: StaticTask<System>,
    mtx: StaticMutex<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System>
            + ~const traits::CfgTask
            + ~const traits::CfgMutex,
    {
        let task0 = StaticTask::define()
            .start(task0_body::<System, D>)
            .priority(0)
            .finish(b);
        let task1 = StaticTask::define()
            .start(task1_body::<System, D>)
            .priority(1)
            .finish(b);
        let task2 = StaticTask::define()
            .start(task2_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);

        let mtx = StaticMutex::define()
            .protocol(MutexProtocol::Ceiling(0))
            .finish(b);
        let seq = Hunk::<_, SeqTracker>::define().finish(b);

        App {
            task0,
            task1,
            task2,
            mtx,
            seq,
        }
    }
}

fn task0_body<System: SupportedSystem, D: Driver<App<System>>>() {
    D::app().seq.expect_and_replace(2, 3);
    D::app().task1.activate().unwrap();

    // Start waiting for `task2` to release `mtx`. Yields CPU to `task1`.
    D::app().seq.expect_and_replace(3, 4);
    D::app().mtx.lock().unwrap();

    D::app().seq.expect_and_replace(6, 7);
    D::app().mtx.unlock().unwrap();
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>() {
    D::app().seq.expect_and_replace(4, 5);

    // Enter a busy loop, indefinitely blocking priority 2.
    while D::app().seq.get() != 7 {}

    D::success();
}

fn task2_body<System: SupportedSystem, D: Driver<App<System>>>() {
    if !MutexProtocol::Ceiling(0).is_supported::<System>() {
        log::warn!("priority ceiling is not supported, skipping the test");
        D::success();
        return;
    }

    assert_eq!(D::app().task2.effective_priority().unwrap(), 2);
    assert_eq!(D::app().task2.priority().unwrap(), 2);

    D::app().seq.expect_and_replace(0, 1);
    D::app().mtx.lock().unwrap();

    // The effective priority is affected by priority ceiling
    assert_eq!(D::app().task2.effective_priority().unwrap(), 0);
    assert_eq!(D::app().task2.priority().unwrap(), 2);

    // Activate `task0`. `task2` is currently running at the same priority as
    // `task0` because of priority ceiling, so this won't cause dispaching.
    //
    // If it weren't for the locking protocol, the following code would dispatch
    // `task0`, which would in turn dispatch `task1`, preventing `task2` from
    // completing the critical section indefinitely.
    D::app().seq.expect_and_replace(1, 2);
    D::app().task0.activate().unwrap();

    // ...until `task2` voluntarily yields CPU.
    System::sleep(Duration::from_millis(200)).unwrap();

    // `task2` is currently running at the same priority as `task0` because of
    // priority ceiling, so `task2` preempts `task1` after sleeping.
    // After unlocking the mutex, `task2`'s priority is restored, and `task0`
    // acquires a mutex lock.
    D::app().seq.expect_and_replace(5, 6);
    D::app().mtx.unlock().unwrap();

    assert_eq!(D::app().task2.effective_priority().unwrap(), 2);
    assert_eq!(D::app().task2.priority().unwrap(), 2);
}
