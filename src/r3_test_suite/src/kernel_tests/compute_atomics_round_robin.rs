//! Performs a atomics-intensive task in multiple tasks that run in a
//! round-robin fashion, and validates the output of each run.
//!
//! The intent of this test to detect bugs in context switching. This test is in
//! particular focused on the use of atomic operations, which are often realized
//! by the combination of load-reserved/store-conditional instructions.
//! Depending on a specific processor implementation, these instructions might
//! behave in an unexpected way if the exclusive flag is not cleared on context
//! switch.
//!
//! # Armv7-A
//!
//! Consider the following program:
//!
//! ```rust,ignore
//! static VAR: AtomicUsize = AtomicUsize::new(0);
//!
//! fn task1() { loop { VAR.fetch_add(1, Ordering::Relaxed); } }
//! fn task2() { /* identical as `task1` */ }
//! ```
//!
//! The following diagram illustrates the pathological situation where the lack
//! of a `clrex` (Clear Exclusive) instruction causes an incorrect behavior:
//!
//! ```text
//!    task1:                           task2:
//!   ────────────────────────────────────────────────────────────────────
//!    ldrex(VAR) → 0
//!                  ┉┉┉┉┉┉┉┉┉┉┉ context switch ┉┉┉┉┉┉┉┉┉┉┉
//!                                     ldrex(VAR) → 0
//!                                     strex(VAR, 0 + 1) → success
//!                                     ldrex(VAR) → 1
//!                                     strex(VAR, 1 + 1) → success
//!                                     ldrex(VAR) → 2
//!                                     strex(VAR, 2 + 1) → success
//!                                     ldrex(VAR) → 3
//!                                     strex(VAR, 3 + 1) → success
//!                                     ldrex(VAR) → 4
//!                  ┉┉┉┉┉┉┉┉┉┉┉ context switch ┉┉┉┉┉┉┉┉┉┉┉
//!    strex(VAR, 0 + 1) → success
//! ```
//!
//! The final value of `VAR` is `1` despite the fact that it was incremented for
//! five times. This can be fixed by executing `clrex` during context switching
//! and thus forcing the last atomic operation to restart after preemption.
//!
//! ```text
//!    task1:                           task2:
//!   ────────────────────────────────────────────────────────────────────
//!    ldrex(VAR) → 0
//!             ┉┉┉┉┉┉┉┉┉┉┉ context switch (+ clrex) ┉┉┉┉┉┉┉┉┉┉┉
//!                                     ldrex(VAR) → 0
//!                                     strex(VAR, 0 + 1) → success
//!                                     ldrex(VAR) → 1
//!                                     strex(VAR, 1 + 1) → success
//!                                     ldrex(VAR) → 2
//!                                     strex(VAR, 2 + 1) → success
//!                                     ldrex(VAR) → 3
//!                                     strex(VAR, 3 + 1) → success
//!                                     ldrex(VAR) → 4
//!             ┉┉┉┉┉┉┉┉┉┉┉ context switch (+ clrex) ┉┉┉┉┉┉┉┉┉┉┉
//!    strex(VAR, 0 + 1) → fail
//!    ldrex(VAR) → 4
//!    strex(VAR, 4 + 1) → success
//! ```
//!
//! # Armv7-M
//!
//! In Armv7-M, the exclusive flag is automatically cleared as part of the
//! exception entry or exit sequence of PendSV, obviating the need for `clrex`
//! during context switching.
//!
//! From *ARMv7-M Architecture Reference Manual*:
//!
//! > In ARMv7-M, the local monitor is changed to Open Access automatically as
//! > part of an exception entry or exit sequence.
//!
//! # Armv8-A
//!
//! Armv8-A removed the need for `clrex` during context switching, assuming
//! application code doesn't voluntarily yield the processor between `ldrex` and
//! `strex`.
//!
//! From *Arm Architecture Reference Manual Armv8, for Armv8-A architecture
//! profile*:
//!
//! > An exception return clears the local monitor. As a result, performing a
//! > CLREX instruction as part of a context switch is not required in most
//! > situations.
use core::{
    cell::UnsafeCell,
    num::Wrapping,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use r3::{
    hunk::Hunk,
    kernel::{traits, Cfg, Task, Timer},
    time::Duration,
    utils::Init,
};

use super::Driver;

const NUM_TASKS: usize = 2;

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
    timer: Timer<System>,
    tasks: [Task<System>; NUM_TASKS],
    judge_task: Task<System>,
    state: Hunk<System, State>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System>
            + ~const traits::CfgTask
            + ~const traits::CfgTimer,
    {
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
                    .priority(2)
                    .param(i)
                    .finish(b),
            );
            i += 1;
        }

        // FIXME: Rewrite this with `<[_; 2]>::map` when it's compatible with `const fn`
        let tasks = [tasks[0].unwrap(), tasks[1].unwrap()];

        let judge_task = Task::build()
            .start(judge_task_body::<System, D>)
            .priority(3)
            .finish(b);

        let state = Hunk::<_, State>::build().finish(b);

        App {
            timer,
            tasks,
            judge_task,
            state,
        }
    }
}

struct State {
    counter: AtomicUsize,
    local_counters: [AtomicUsize; NUM_TASKS],
    stop: AtomicBool,

    sched_state: UnsafeCell<SchedState>,
}

struct SchedState {
    cur_task: usize,
    time: usize,
}

unsafe impl Sync for State {}

impl Init for State {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self {
        counter: Init::INIT,
        local_counters: Init::INIT,
        stop: Init::INIT,
        sched_state: Init::INIT,
    };
}

impl Init for SchedState {
    const INIT: Self = Self {
        cur_task: 0,
        time: 0,
    };
}

fn worker_body<System: SupportedSystem, D: Driver<App<System>>>(worker_id: usize) {
    let App { state, .. } = D::app();

    let mut local_counter = Wrapping(0usize);

    while !state.stop.load(Ordering::Relaxed) {
        match () {
            #[cfg(target_has_atomic = "ptr")]
            () => {
                // `fetch_update` is realized by LR/SC on Arm and RISC-V
                state
                    .counter
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |x| Some(x + 1))
                    .unwrap();
            }

            #[cfg(not(target_has_atomic = "ptr"))]
            () => {
                use r3::kernel::prelude::*;
                System::acquire_cpu_lock().unwrap();
                state.counter.store(
                    state.counter.load(Ordering::Relaxed).wrapping_add(1),
                    Ordering::Relaxed,
                );
                unsafe { System::release_cpu_lock().unwrap() };
            }
        }

        local_counter += Wrapping(1);
        state.local_counters[worker_id].store(local_counter.0, Ordering::Relaxed);
    }
}

fn judge_task_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    let App { state, .. } = D::app();

    let counter = state.counter.load(Ordering::Relaxed);
    let mut local_counters = [0; NUM_TASKS];
    for (x, y) in state.local_counters.iter().zip(local_counters.iter_mut()) {
        *y = x.load(Ordering::Relaxed);
    }

    let Wrapping(local_counter_sum) = local_counters.iter().cloned().map(Wrapping).sum();

    log::debug!("counter = {}", counter);
    log::debug!(
        "local_counters = {:?} (sum = {})",
        local_counters,
        local_counter_sum
    );

    assert_eq!(counter, local_counter_sum);

    D::success();
}

fn timer_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    let App {
        state,
        tasks,
        judge_task,
        timer,
        ..
    } = D::app();

    // Safety: This is a unique reference
    let sched_state = unsafe { &mut *state.sched_state.get() };

    sched_state.time += 1;

    // Switch the running task
    let new_task = (sched_state.cur_task + 1) % NUM_TASKS;
    log::trace!("scheduing tasks[{}]", new_task);
    tasks[sched_state.cur_task].set_priority(2).unwrap();
    tasks[new_task].set_priority(1).unwrap();
    sched_state.cur_task = new_task;

    // Wait for several ticks to increase the probability of catching any bugs
    if sched_state.time < 100 {
        return;
    }

    log::debug!("Stopping the workers");
    timer.stop().unwrap();
    state.stop.store(true, Ordering::Relaxed);

    // Run `judge_task_body` after all worker tasks stop running.
    judge_task.activate().unwrap();
}
