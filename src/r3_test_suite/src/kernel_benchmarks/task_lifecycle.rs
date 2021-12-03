//! Measures the execution times of task lifecycle operations.
//!
//! ```text
//!          main               task1              task2
//!           │ │                 ┊                  ┊
//!           │ │    activate     ┊                  ┊     ┐
//!           └┬┘ ─────────────► ┌┴┐                 ┊     │ I_ACTIVATE_DISPATCHING
//!            │                 │ │                 ┊     ┘
//!            │                 │ │    activate     ┊     ┐
//!            │                 │ │ ─────────────►  ┊     │ I_ACTIVATE
//!            │                 │ │                 ┊     ┘
//!            │                 │ │ exit by return  ┊     ┐
//!            │                 └┬┘ ─────────────► ┌┴┐    │ I_EXIT_BY_RETURN
//!            │                  ┊                 │ │    ┘
//!            │                  ┊   exit_task     │ │    ┐
//!           ┌┴┐ ◀──────────────────────────────── └┬┘    │ I_EXIT
//!           │ │                 ┊                  ┊     ┘
//! ```
//!
use r3::kernel::{prelude::*, traits, Cfg, Task};

use super::Bencher;
use crate::utils::benchmark::Interval;

use_benchmark_in_kernel_benchmark! {
    pub unsafe struct App<System: SupportedSystem> {
        inner: AppInner<System>,
    }
}

pub trait SupportedSystem: crate::utils::benchmark::SupportedSystem {}
impl<T: crate::utils::benchmark::SupportedSystem> SupportedSystem for T {}

struct AppInner<System: SupportedSystem> {
    task1: Task<System>,
    task2: Task<System>,
}

const I_ACTIVATE_DISPATCHING: Interval = "activating task with dispatch";
const I_ACTIVATE: Interval = "activating task";
const I_EXIT_BY_RETURN: Interval = "exiting task by returning";
const I_EXIT: Interval = "exiting task by `exit_task`";

impl<System: SupportedSystem> AppInner<System> {
    /// Used by `use_benchmark_in_kernel_benchmark!`
    const fn new<C, B: Bencher<System, Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System> + ~const traits::CfgTask,
    {
        let task1 = Task::define()
            .start(task1_body::<System, B>)
            .priority(1)
            .finish(b);

        let task2 = Task::define()
            .start(task2_body::<System, B>)
            .priority(2)
            .finish(b);

        Self { task1, task2 }
    }

    /// Used by `use_benchmark_in_kernel_benchmark!`
    fn iter<B: Bencher<System, Self>>() {
        B::mark_start();
        B::app().task1.activate().unwrap();
        B::mark_end(I_EXIT);
    }
}

fn task1_body<System: SupportedSystem, B: Bencher<System, AppInner<System>>>(_: usize) {
    B::mark_end(I_ACTIVATE_DISPATCHING);
    B::mark_start();
    B::app().task2.activate().unwrap();
    B::mark_end(I_ACTIVATE);
    B::mark_start();
}

fn task2_body<System: SupportedSystem, B: Bencher<System, AppInner<System>>>(_: usize) {
    B::mark_end(I_EXIT_BY_RETURN);
    B::mark_start();
    unsafe { System::exit_task().unwrap() };
}
