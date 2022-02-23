//! Performs a compute-intensive task in multiple tasks that run in a
//! round-robin fashion, and validates the output of each run.
//!
//! The intent of this test to detect bugs in the saving and restoring of
//! registers during a context switch. To this end, the task is designed to
//! utilize as many floating-point registers as possible.
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use r3::{
    bind::Bind,
    hunk::Hunk,
    kernel::{prelude::*, traits, Cfg, StaticTask, StaticTimer},
    prelude::*,
    time::Duration,
    utils::Init,
};

use super::Driver;
use crate::utils::compute;

const NUM_TASKS: usize = 3;

pub trait SupportedSystem:
    traits::KernelBase + traits::KernelTaskSetPriority + traits::KernelTimer + traits::KernelStatic
{
}
impl<
        T: traits::KernelBase
            + traits::KernelTaskSetPriority
            + traits::KernelTimer
            + traits::KernelStatic,
    > SupportedSystem for T
{
}

pub struct App<System: SupportedSystem> {
    timer: StaticTimer<System>,
    tasks: [StaticTask<System>; NUM_TASKS],
    state: Hunk<System, State>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System>
            + ~const traits::CfgTask
            + ~const traits::CfgTimer,
    {
        let task_state = Bind::define().init(|| TaskState::INIT).finish(b);
        let ref_output = Bind::define()
            .init_with_bind((task_state.borrow_mut(),), generate_ref_output)
            .finish(b);

        let sched_state = Bind::define().init(|| SchedState::INIT).finish(b);

        let timer = StaticTimer::define()
            .delay(Duration::from_millis(0))
            .period(Duration::from_millis(if cfg!(target_os = "none") {
                10
            } else {
                50
            }))
            .start_with_bind((sched_state.borrow_mut(),), timer_body::<System, D>)
            .active(true)
            .finish(b);

        let mut tasks = [None; NUM_TASKS];

        // FIXME: Work-around for `for` being unsupported in `const fn`
        let mut i = 0;
        while i < NUM_TASKS {
            let task_state = if i == 0 {
                // Reuse the storage
                task_state.borrow_mut()
            } else {
                Bind::define()
                    .init(|| TaskState::INIT)
                    .finish(b)
                    .borrow_mut()
            };

            tasks[i] = Some(
                StaticTask::define()
                    .active(true)
                    .start_with_bind(
                        (task_state, ref_output.borrow()),
                        move |task_state: &mut _, ref_output: &_| {
                            worker_body::<System, D>(task_state, ref_output, i)
                        },
                    )
                    .priority(3)
                    .finish(b),
            );
            i += 1;
        }

        // FIXME: Rewrite this with `<[_; 4]>::map` when it's compatible with `const fn`
        let tasks = [tasks[0].unwrap(), tasks[1].unwrap(), tasks[2].unwrap()];

        let state = Hunk::<_, State>::define().finish(b);

        App {
            timer,
            tasks,
            state,
        }
    }
}

struct State {
    /// The number of times the workload was completed by each task.
    run_count: [AtomicUsize; NUM_TASKS],
    stop: AtomicBool,
}

struct TaskState {
    kernel_state: compute::KernelState,
    output: compute::KernelOutput,
}

struct SchedState {
    cur_task: usize,
    time: usize,
}

impl Init for State {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self {
        run_count: Init::INIT,
        stop: Init::INIT,
    };
}

impl Init for TaskState {
    #[allow(clippy::declare_interior_mutable_const)] // it's intentional
    const INIT: Self = Self {
        kernel_state: Init::INIT,
        output: Init::INIT,
    };
}

impl Init for SchedState {
    const INIT: Self = Self {
        cur_task: 0,
        time: 0,
    };
}

fn generate_ref_output(task_state: &mut TaskState) -> compute::KernelOutput {
    let mut out = Init::INIT;
    task_state.kernel_state.run(&mut out);
    out
}

#[inline]
fn worker_body<System: SupportedSystem, D: Driver<App<System>>>(
    task_state: &mut TaskState,
    ref_output: &compute::KernelOutput,
    worker_id: usize,
) {
    let App { state, .. } = D::app();

    let run_count = &state.run_count[worker_id];

    let mut i = 0;

    while !state.stop.load(Ordering::Relaxed) {
        i += 1;
        log::trace!("[{}] Iteration {}: starting", worker_id, i);
        task_state.output = Init::INIT;

        // Run the computation
        task_state.kernel_state.run(&mut task_state.output);

        // Validate the output
        log::trace!("[{}] Iteration {}: validating", worker_id, i);
        let valid = task_state.output == *ref_output;
        if !valid {
            stop::<System, D>();
            panic!("Output validation failed");
        }

        log::trace!("[{}] Iteration {}: complete", worker_id, i);

        // Note: Some targets don't support CAS atomics. Non-atomic load/store
        //       suffices because `run_count` is only written by this task.
        run_count.store(run_count.load(Ordering::Relaxed) + 1, Ordering::Relaxed);
    }
}

fn timer_body<System: SupportedSystem, D: Driver<App<System>>>(sched_state: &mut SchedState) {
    let App { state, tasks, .. } = D::app();

    sched_state.time += 1;

    // Switch the running task
    let new_task = (sched_state.cur_task + 1) % NUM_TASKS;
    log::trace!("scheduing tasks[{}]", new_task);
    tasks[sched_state.cur_task].set_priority(3).unwrap();
    tasks[new_task].set_priority(2).unwrap();
    sched_state.cur_task = new_task;

    // Wait for several ticks to catch any bugs in context switching
    if sched_state.time < 100 {
        return;
    }

    // Check the run count of tasks
    let mut run_count = [0; NUM_TASKS];
    for (x, y) in state.run_count.iter().zip(run_count.iter_mut()) {
        *y = x.load(Ordering::Relaxed);
    }

    if sched_state.time % 20 == 0 {
        log::debug!("run_count = {:?}", run_count);
    }

    let min_run_count: usize = *run_count.iter().min().unwrap();
    let max_run_count: usize = *run_count.iter().max().unwrap();

    // Ensure the workload runs for sufficient times
    if max_run_count < 3 {
        if sched_state.time > 4000 {
            // Timeout
            stop::<System, D>();
            panic!("Timeout");
        }
        return;
    }

    // Tasks are scheduled in a round-robin fashion. If there's a task that has
    // never completed the workload, something is wrong.
    if min_run_count == 0 {
        stop::<System, D>();
        panic!(
            "Too much inbalance between tasks - round-robin scheduling \
            might not be working"
        );
    }

    log::info!("Success!");
    stop::<System, D>();
    D::success();
}

fn stop<System: SupportedSystem, D: Driver<App<System>>>() {
    let App { state, timer, .. } = D::app();

    log::debug!("Stopping the workers");
    timer.stop().unwrap();
    state.stop.store(true, Ordering::Relaxed);
}
