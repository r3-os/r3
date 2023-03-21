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
    num::Wrapping,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use r3::{
    bind::Bind,
    hunk::Hunk,
    kernel::{prelude::*, traits, Cfg, StaticTask, StaticTimer},
    prelude::*,
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
    timer: StaticTimer<System>,
    tasks: [StaticTask<System>; NUM_TASKS],
    judge_task: StaticTask<System>,
    state: Hunk<System, State>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgTask<System = System> + ~const traits::CfgTimer,
    {
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

        // `[T]::iter_mut` is unusable in `const fn` [ref:const_slice_iter]
        // `core::array::from_fn` is not `const fn` [ref:const_array_from_fn]
        // FIXME: `needless_range_loop` false positive
        // <https://github.com/rust-lang/rust-clippy/issues/10524>
        #[expect(clippy::needless_range_loop)]
        for i in 0..NUM_TASKS {
            tasks[i] = Some(
                StaticTask::define()
                    .active(true)
                    .start((i, worker_body::<System, D>))
                    .priority(2)
                    .finish(b),
            );
        }

        // `<[_; 2]>::map` is unusable in `const fn` [ref:const_array_map]
        let tasks = [tasks[0].unwrap(), tasks[1].unwrap()];

        let judge_task = StaticTask::define()
            .start(judge_task_body::<System, D>)
            .priority(3)
            .finish(b);

        let state = Hunk::<_, State>::define().finish(b);

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
}

struct SchedState {
    cur_task: usize,
    time: usize,
}

impl Init for State {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self {
        counter: Init::INIT,
        local_counters: Init::INIT,
        stop: Init::INIT,
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

fn judge_task_body<System: SupportedSystem, D: Driver<App<System>>>() {
    let App { state, .. } = D::app();

    let counter = state.counter.load(Ordering::Relaxed);
    let mut local_counters = [0; NUM_TASKS];
    for (x, y) in state.local_counters.iter().zip(local_counters.iter_mut()) {
        *y = x.load(Ordering::Relaxed);
    }

    let Wrapping(local_counter_sum) = local_counters.iter().cloned().map(Wrapping).sum();

    log::debug!("counter = {counter}");
    log::debug!("local_counters = {local_counters:?} (sum = {local_counter_sum})");

    assert_eq!(counter, local_counter_sum);

    D::success();
}

fn timer_body<System: SupportedSystem, D: Driver<App<System>>>(sched_state: &mut SchedState) {
    let App {
        state,
        tasks,
        judge_task,
        timer,
        ..
    } = D::app();

    sched_state.time += 1;

    // Switch the running task
    let new_task = (sched_state.cur_task + 1) % NUM_TASKS;
    log::trace!("scheduing tasks[{new_task}]");
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
