//! Configures `Timer` as a one-shot timer, stops it for a brief moment, and
//! checks that it fires at expected moments.
//!
//! ```text
//!       __          __    __          __         __
//! Task |__|        |__|  |__|        |__|       |__|
//!      0→1         1→2   2→3         3→4        5→6
//!                                              _
//! Timer callback                              |_|
//!                                             4→5
//!      ├──┬──┬──┬──┼──┬──┼──┬──┬──┬──┼──┬──┬──┤
//!      ↑   400ms   ↑ 200ms   400ms     300ms
//! system boot  set delay
//!               (500ms)  ↑           ↑
//!                       stop       start
//! ```
use r3::{
    hunk::Hunk,
    kernel::{prelude::*, traits, Cfg, StaticTask, StaticTimer},
    time::Duration,
};

use super::Driver;
use crate::utils::{conditional::KernelTimeExt, SeqTracker};

pub trait SupportedSystem:
    traits::KernelBase + traits::KernelTimer + traits::KernelStatic + KernelTimeExt
{
}
impl<T: traits::KernelBase + traits::KernelTimer + traits::KernelStatic + KernelTimeExt>
    SupportedSystem for T
{
}

pub struct App<System: SupportedSystem> {
    timer: StaticTimer<System>,
    task: StaticTask<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgTask<System = System> + ~const traits::CfgTimer,
    {
        let timer = StaticTimer::define()
            .active(true)
            .start(timer_body::<System, D>)
            .finish(b);

        let task = StaticTask::define()
            .active(true)
            .start(task_body::<System, D>)
            .priority(1)
            .finish(b);

        let seq = Hunk::<_, SeqTracker>::define().finish(b);

        App { timer, task, seq }
    }
}

fn task_body<System: SupportedSystem, D: Driver<App<System>>>() {
    let App { seq, timer, .. } = D::app();

    // Expected current time
    let mut now = 0;

    seq.expect_and_replace(0, 1);

    System::sleep_ms(400);
    now += 400;

    macro_rules! check_time {
        () => {
            System::assert_time_ms_range(now..now + 100);
        };
    }

    // Start the timer
    check_time!();
    seq.expect_and_replace(1, 2);
    timer.set_delay(Some(Duration::from_millis(500))).unwrap();

    System::sleep_ms(200);
    now += 200;

    // Suspend the timer
    seq.expect_and_replace(2, 3);
    timer.stop().unwrap();
    check_time!();

    System::sleep_ms(400);
    now += 400;

    // Resume the timer
    seq.expect_and_replace(3, 4);
    timer.start().unwrap();
    check_time!();

    // Tick
    System::park().unwrap();
    seq.expect_and_replace(5, 6);
    now += 300;
    check_time!();

    D::success();
}

fn timer_body<System: SupportedSystem, D: Driver<App<System>>>() {
    let App { task, seq, .. } = D::app();

    seq.expect_and_replace(4, 5);
    task.unpark_exact().unwrap();
}
