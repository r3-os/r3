//! The common part of `mutex_*`. See [`super::mutex_none`] for a sequence
//! diagram.
use constance::{
    kernel::{cfg::CfgBuilder, Kernel, Mutex, MutexProtocol, Task},
    time::Duration,
};
use core::marker::PhantomData;

use super::Bencher;
use crate::utils::benchmark::Interval;

pub(super) struct AppInner<System, Options> {
    task1: Task<System>,
    mtx: Mutex<System>,
    _phantom: PhantomData<Options>,
}

pub(super) trait MutexBenchmarkOptions: 'static + Send + Sync {
    const PROTOCOL: MutexProtocol;
}

pub(super) type AppInnerNone<System> = AppInner<System, NoneOptions>;
pub(super) type AppInnerCeiling<System> = AppInner<System, CeilingOptions>;

pub(super) struct NoneOptions;
pub(super) struct CeilingOptions;

impl MutexBenchmarkOptions for NoneOptions {
    const PROTOCOL: MutexProtocol = MutexProtocol::None;
}

impl MutexBenchmarkOptions for CeilingOptions {
    const PROTOCOL: MutexProtocol = MutexProtocol::Ceiling(1);
}

const I_LOCK: Interval = "lock mutex";
const I_UNLOCK_DISPATCHING: Interval = "unlock mutex with dispatch";
const I_UNLOCK: Interval = "unlock mutex";

impl<System: Kernel, Options: MutexBenchmarkOptions> AppInner<System, Options> {
    /// Used by `use_benchmark_in_kernel_benchmark!`
    pub(super) const fn new<B: Bencher<Self>>(b: &mut CfgBuilder<System>) -> Self {
        let task1 = Task::build()
            .start(task1_body::<System, Options, B>)
            .priority(1)
            .finish(b);

        let mtx = Mutex::build().protocol(Options::PROTOCOL).finish(b);

        Self {
            task1,
            mtx,
            _phantom: PhantomData,
        }
    }

    /// Used by `use_benchmark_in_kernel_benchmark!`
    pub(super) fn iter<B: Bencher<Self>>() {
        B::mark_start(); // I_LOCK
        B::app().mtx.lock().unwrap();
        B::mark_end(I_LOCK);

        B::app().task1.activate().unwrap();
        System::sleep(Duration::from_millis(200)).unwrap();

        B::mark_start(); // I_UNLOCK_DISPATCHING
        B::app().mtx.unlock().unwrap();
    }
}

fn task1_body<
    System: Kernel,
    Options: MutexBenchmarkOptions,
    B: Bencher<AppInner<System, Options>>,
>(
    _: usize,
) {
    B::app().mtx.lock().unwrap();
    B::mark_end(I_UNLOCK_DISPATCHING);

    B::mark_start();
    B::app().mtx.unlock().unwrap();
    B::mark_end(I_UNLOCK);
}
