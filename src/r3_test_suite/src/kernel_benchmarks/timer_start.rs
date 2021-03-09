//! Measures the execution times taken to start timers.
use core::mem::MaybeUninit;

use r3::{
    kernel::{cfg::CfgBuilder, Kernel, Timer},
    time::Duration,
};

use super::Bencher;
use crate::utils::benchmark::Interval;

use_benchmark_in_kernel_benchmark! {
    pub unsafe struct App<System> {
        inner: AppInner<System>,
    }
}

struct AppInner<System> {
    timers: [Timer<System>; 64],
}

const I_START_1: Interval = "start the 1st timer";
const I_START_2: Interval = "start the 2nd timer";
const I_START_4: Interval = "start the 4th timer";
const I_START_8: Interval = "start the 8th timer";
const I_START_16: Interval = "start the 16th timer";
const I_START_32: Interval = "start the 32nd timer";
const I_START_64: Interval = "start the 64th timer";

impl<System: Kernel> AppInner<System> {
    /// Used by `use_benchmark_in_kernel_benchmark!`
    const fn new<B: Bencher<System, Self>>(b: &mut CfgBuilder<System>) -> Self {
        let timers = {
            let mut timers = [MaybeUninit::<Timer<System>>::uninit(); 64];

            let mut i = 0;
            // FIXME: Work-around for `for` being unsupported in `const fn`
            while i < timers.len() {
                timers[i] = MaybeUninit::new(Timer::build().start(|_| {}).finish(b));
                i += 1;
            }

            // FIXME: use <https://github.com/rust-lang/rust/issues/80908> when
            //        it becomes `const fn`
            unsafe { core::mem::transmute(timers) }
        };

        Self { timers }
    }

    /// Used by `use_benchmark_in_kernel_benchmark!`
    fn iter<B: Bencher<System, Self>>() {
        let timers = &B::app().timers;

        // Reset the timers
        for (i, timer) in timers.iter().enumerate() {
            // cause the worst-case binary heap insertion
            timer
                .set_delay(Some(Duration::from_secs((timers.len() - i) as i32)))
                .unwrap();
        }

        // Start the timers and insert them to the timeout heap
        let mut i = 0;
        for &(interval_i, interval) in &[
            (0, I_START_1),
            (1, I_START_2),
            (3, I_START_4),
            (7, I_START_8),
            (15, I_START_16),
            (31, I_START_32),
            (63, I_START_64),
        ] {
            while i < interval_i {
                timers[i].start().unwrap();
                i += 1;
            }

            B::mark_start(); // I_WAIT
            timers[i].start().unwrap();
            B::mark_end(interval);
            i += 1;
        }

        // Stop all the timers
        for timer in timers.iter() {
            timer.stop().unwrap();
        }
    }
}
