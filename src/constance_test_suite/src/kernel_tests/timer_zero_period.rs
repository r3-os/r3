//! Configures `Timer` as a periodic timer with `period = 0`, checks that the
//! system calls the callback function repeatedly until `period` and `delay` are
//! set to different values or the timer is stopped.
//!
//! ```text
//!                 _       _           _       _
//! Task           |_|     |_|         |_|     |_|
//!                        3→4         5→6     9→10
//!                   _ _ _           _   _ _ _
//! Timer callback   |_|_|_|         |_| |_|_|_|
//!                  0→1→2→3         4→5 6→7→8→9
//!                ├────────┬──┬──┬──┼────────────┬──┬──┬──┤
//!                ↑       400ms              400ms
//!     (1) system boot  ↑ (2)          ↑ (3)
//!                delay ← 400ms   delay ← 0
//!                 period ← ∞    period ← 0  ↑ (4)
//!                                          stop
//! ```
//!
//!  1. The timer is configured with `(period, delay) = (0, 0)`. The system
//!     starts calling the callback function repeatedly. This wouldn't stop if
//!     it had gone uninterrupted.
//!
//!  2. Our callback function, when called for the third time, stops this
//!     by changing `period` and `delay` of the timer. The timer interrupt
//!     handler can now return and the task is given a change to execute.
//!     The next tick happens normally.
//!
//!  3. The task reconfigures the timer with `(period, delay) = (0, 0)`. The
//!     system again starts calling the callback function repeatedly.
//!
//!  4. Our callback function, when called for the seventh time, stops this
//!     by calling `Timer::stop` on the timer. The timer interrupt
//!     handler can now return and the task is given a change to execute.
//!
use constance::{
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
            .delay(Duration::from_millis(0))
            .period(Duration::from_millis(0))
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

    // Wait until the system finishes the first batch of ticks
    System::park().unwrap();
    seq.expect_and_replace(3, 4);

    System::assert_time_ms_range(0..100);

    // The next tick
    System::park().unwrap();
    seq.expect_and_replace(5, 6);

    System::assert_time_ms_range(400..500);

    // Set the period to zero again
    timer.set_period(Some(Duration::ZERO)).unwrap();
    timer.set_delay(Some(Duration::ZERO)).unwrap();

    // The last three ticks
    System::park().unwrap();
    seq.expect_and_replace(9, 10);

    System::assert_time_ms_range(400..500);

    // Make sure that was the last tick
    System::sleep_ms(500);

    D::success();
}

fn timer_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let App {
        timer, task, seq, ..
    } = D::app();

    match seq.get() {
        0 => {
            seq.expect_and_replace(0, 1);
        }
        1 => {
            seq.expect_and_replace(1, 2);
        }
        2 => {
            seq.expect_and_replace(2, 3);
            timer.set_delay(Some(Duration::from_millis(400))).unwrap();
            timer.set_period(None).unwrap();
            task.unpark_exact().unwrap();
        }

        // 400ms
        4 => {
            seq.expect_and_replace(4, 5);
            task.unpark_exact().unwrap();
        }

        // 400ms
        6 => {
            seq.expect_and_replace(6, 7);
            task.unpark_exact().unwrap();
        }
        7 => {
            seq.expect_and_replace(7, 8);
        }
        8 => {
            seq.expect_and_replace(8, 9);
            timer.stop().unwrap();
        }

        _ => unreachable!(),
    }
}
