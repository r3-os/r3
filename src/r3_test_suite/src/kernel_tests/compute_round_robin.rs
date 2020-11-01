//! Performs a compute-intensive task in multiple tasks that run in a
//! round-robin fashion, and validates the output of each run.
//!
//! The intent of this test to detect bugs in the saving and restoring of
//! registers during a context switch. To this end, the task is designed to
//! utilize as many floating-point registers as possible.
use r3::{
    hunk::Hunk,
    kernel::{cfg::CfgBuilder, StartupHook, Task, Timer},
    prelude::*,
    time::Duration,
    utils::Init,
};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use super::Driver;
use crate::utils::compute;

const NUM_TASKS: usize = 3;

pub struct App<System> {
    timer: Timer<System>,
    tasks: [Task<System>; NUM_TASKS],
    state: Hunk<System, State>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        StartupHook::build().start(hook_body::<System, D>).finish(b);

        let timer = Timer::build()
            .delay(Duration::from_millis(0))
            .period(Duration::from_millis(10))
            .start(timer_body::<System, D>)
            .active(true)
            .finish(b);

        let mut tasks = [None; NUM_TASKS];

        // FIXME: Work-around for `for` being unsupported in `const fn`
        let mut i = 0;
        while i < NUM_TASKS {
            tasks[i] = Some(
                Task::build()
                    .active(true)
                    .start(worker_body::<System, D>)
                    .priority(3)
                    .param(i)
                    .finish(b),
            );
            i += 1;
        }

        // FIXME: Rewrite this with `<[_; 4]>::map` when it's compatible with `const fn`
        let tasks = [tasks[0].unwrap(), tasks[1].unwrap(), tasks[2].unwrap()];

        let state = Hunk::<_, State>::build().finish(b);

        App {
            timer,
            tasks,
            state,
        }
    }
}

struct State {
    ref_output: UnsafeCell<compute::KernelOutput>,
    task_state: [UnsafeCell<TaskState>; NUM_TASKS],

    /// The number of times the workload was completed by each task.
    run_count: [AtomicUsize; NUM_TASKS],
    stop: AtomicBool,

    sched_state: UnsafeCell<SchedState>,
}

struct TaskState {
    kernel_state: compute::KernelState,
    output: compute::KernelOutput,
}

struct SchedState {
    cur_task: usize,
    time: usize,
}

unsafe impl Sync for State {}

impl Init for State {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self {
        ref_output: Init::INIT,
        task_state: Init::INIT,
        run_count: Init::INIT,
        stop: Init::INIT,
        sched_state: Init::INIT,
    };
}

impl Init for TaskState {
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

fn hook_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let state = &*D::app().state;

    // Safety: These are unique references to the contents of respective cells
    let ref_output = unsafe { &mut *state.ref_output.get() };
    let task_state = unsafe { &mut *state.task_state[0].get() };

    // Obtain the refernce output
    task_state.kernel_state.run(ref_output);
}

fn worker_body<System: Kernel, D: Driver<App<System>>>(worker_id: usize) {
    let App { state, .. } = D::app();

    // Safety: This is a unique reference
    let task_state = unsafe { &mut *state.task_state[worker_id].get() };

    // Safety: A mutable reference to `ref_output` doesn't exist at this point
    let ref_output = unsafe { &*state.ref_output.get() };

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

fn timer_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let App { state, tasks, .. } = D::app();

    // Safety: This is a unique reference
    let sched_state = unsafe { &mut *state.sched_state.get() };

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

fn stop<System: Kernel, D: Driver<App<System>>>() {
    let App { state, timer, .. } = D::app();

    log::debug!("Stopping the workers");
    timer.stop().unwrap();
    state.stop.store(true, Ordering::Relaxed);
}
