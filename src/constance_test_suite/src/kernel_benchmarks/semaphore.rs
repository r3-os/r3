//! Measures the execution times of semaphore operations.
//!
//! ```text
//!      sem      main               task1
//!      │1│       │ │                 ┊
//!      │ │       │ │                 ┊     ┐
//!      ├─┤       │ │ sem wait        ┊     │ I_WAIT
//!      │0│       │ │                 ┊     ┘
//!      │ │       │ │    activate     ┊    
//!      │ │       └┬┘ ─────────────► ┌┴┐
//!      │ │        │                 │ │
//!      │ │        │     sem wait    │ │    ┐
//!      │ │       ┌┴┐ ◀────────────  └┬┘    │ I_WAIT_DISPATCING
//!      │ │       │ │                 │     ┘
//!      │ │       │ │   sem signal    │     ┐
//!      ├─┤       └┬┘ ─────────────► ┌┴┐    │ I_SIGNAL_DISPATCING
//!      │0│        ┊                 │ │    ┘
//!      │ │        ┊                 │ │             ┐
//!      ├─┤        ┊                 │ │ sem signal  │ I_SIGNAL
//!      │1│        ┊    exit_task    │ │             ┘
//!      │ │       ┌┴┐ ◀───────────── └┬┘
//!      │ │       │ │                 ┊
//! ```
//!
use constance::kernel::{cfg::CfgBuilder, Kernel, Semaphore, Task};

use super::Bencher;
use crate::utils::benchmark::Interval;

use_benchmark_in_kernel_benchmark! {
    pub unsafe struct App<System> {
        inner: AppInner<System>,
    }
}

struct AppInner<System> {
    task1: Task<System>,
    sem: Semaphore<System>,
}

const I_WAIT_DISPATCHING: Interval = "wait semaphore with dispatch";
const I_WAIT: Interval = "wait semaphore";
const I_SIGNAL_DISPATCHING: Interval = "signal semaphore with dispatch";
const I_SIGNAL: Interval = "signal semaphore";

impl<System: Kernel> AppInner<System> {
    /// Used by `use_benchmark_in_kernel_benchmark!`
    const fn new<B: Bencher<Self>>(b: &mut CfgBuilder<System>) -> Self {
        let task1 = Task::build()
            .start(task1_body::<System, B>)
            .priority(1)
            .finish(b);

        let sem = Semaphore::build().initial(1).maximum(1).finish(b);

        Self { task1, sem }
    }

    /// Used by `use_benchmark_in_kernel_benchmark!`
    fn iter<B: Bencher<Self>>() {
        B::mark_start(); // I_WAIT
        B::app().sem.wait_one().unwrap();
        B::mark_end(I_WAIT);

        B::app().task1.activate().unwrap();
        B::mark_end(I_WAIT_DISPATCHING);

        B::mark_start(); // I_SIGNAL_DISPATCHING
        B::app().sem.signal_one().unwrap();
    }
}

fn task1_body<System: Kernel, B: Bencher<AppInner<System>>>(_: usize) {
    B::mark_start(); // I_WAIT_DISPATCHING
    B::app().sem.wait_one().unwrap();
    B::mark_end(I_SIGNAL_DISPATCHING);

    B::mark_start();
    B::app().sem.signal_one().unwrap();
    B::mark_end(I_SIGNAL);
}
