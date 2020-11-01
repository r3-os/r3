//! Configures `Timer` as a periodic timer, advances the time by `adjust_time`,
//! and checks that the system calls the callback function for all overdue
//! ticks.
//!
//! ```text
//!
//!                            adjust_time (130ms)
//!                 ------------------------------------->
//!                 ______________________________________       _          _
//! Task           |______________________________________|     |_|        |_|
//!                0→1                                          4→5        6→7
//!                                                        _ _ _          _
//! Timer callback                                        |_|_|_|        |_|
//!                                                       1→2→3→4        5→6
//!                ├──┬──┬──┬──┼──┬──┬──┬──┼──┬──┬──┬──┼──┬────────┬──┬──┼─────
//!                ↑   400ms       400ms       400ms             400ms
//!          system boot
//! ```
use r3::{
    hunk::Hunk,
    kernel::{cfg::CfgBuilder, Task, Timer},
    prelude::*,
    time::Duration,
};

use super::Driver;
use crate::utils::{time::KernelTimeExt, SeqTracker};

pub struct App<System> {
    timer: Timer<System>,
    task: Task<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        let timer = Timer::build()
            .delay(Duration::from_millis(400))
            .period(Duration::from_millis(400))
            .active(true)
            .start(timer_body::<System, D>)
            .param(42)
            .finish(b);

        let task = Task::build()
            .active(true)
            .start(task_body::<System, D>)
            .priority(1)
            .finish(b);

        let seq = Hunk::<_, SeqTracker>::build().finish(b);

        App { timer, task, seq }
    }
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let App { seq, timer, .. } = D::app();

    seq.expect_and_replace(0, 1);

    // Advance the time
    System::adjust_time(Duration::from_millis(1300)).unwrap();

    // Now the system has missed three calls to the callback function.
    // The system will process them soon. (It's unspecified whether it
    // happens in `adjust_time`)

    // Wait until the system finishes processing the overdue calls
    System::park().unwrap();
    seq.expect_and_replace(4, 5);

    System::assert_time_ms_range(1300..1400);

    // The final tick, which takes place on time
    System::park().unwrap();
    seq.expect_and_replace(6, 7);

    System::assert_time_ms_range(1600..1700);

    timer.stop().unwrap();

    D::success();
}

fn timer_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let App { task, seq, .. } = D::app();

    match seq.get() {
        1 => {
            seq.expect_and_replace(1, 2);
        }
        2 => {
            seq.expect_and_replace(2, 3);
        }
        3 => {
            seq.expect_and_replace(3, 4);
            task.unpark_exact().unwrap();
        }
        5 => {
            seq.expect_and_replace(5, 6);
            task.unpark_exact().unwrap();
        }
        _ => unreachable!(),
    }
}
