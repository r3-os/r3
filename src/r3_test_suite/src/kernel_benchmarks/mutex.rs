//! The common part of `mutex_*`. See [`super::mutex_none`] for a sequence
//! diagram.
use core::marker::PhantomData;
use r3::kernel::{traits, Cfg, Mutex, MutexProtocol, Task};

use super::Bencher;
use crate::utils::benchmark::Interval;

pub trait SupportedSystem: crate::utils::benchmark::SupportedSystem + traits::KernelMutex {}
impl<T: crate::utils::benchmark::SupportedSystem + traits::KernelMutex> SupportedSystem for T {}

pub(super) struct AppInner<System: SupportedSystem, Options> {
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

impl<System: SupportedSystem, Options: MutexBenchmarkOptions> AppInner<System, Options> {
    /// Used by `use_benchmark_in_kernel_benchmark!`
    pub(super) const fn new<C, B: Bencher<System, Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System>
            + ~const traits::CfgTask
            + ~const traits::CfgMutex,
    {
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
    pub(super) fn iter<B: Bencher<System, Self>>() {
        B::mark_start(); // I_LOCK
        B::app().mtx.lock().unwrap();
        B::mark_end(I_LOCK);

        B::app().task1.activate().unwrap();
        System::park().unwrap();

        B::mark_start(); // I_UNLOCK_DISPATCHING
        B::app().mtx.unlock().unwrap();
    }
}

fn task1_body<
    System: SupportedSystem,
    Options: MutexBenchmarkOptions,
    B: Bencher<System, AppInner<System, Options>>,
>(
    _: usize,
) {
    B::main_task().unpark().unwrap();
    B::app().mtx.lock().unwrap();
    B::mark_end(I_UNLOCK_DISPATCHING);

    B::mark_start();
    B::app().mtx.unlock().unwrap();
    B::mark_end(I_UNLOCK);
}
