//! Checks miscellaneous properties of `Timer`.
#[cfg(feature = "system_time")]
use r3::time::Time;
use r3::{
    hunk::Hunk,
    kernel::{self, cfg::CfgBuilder, Task, Timer},
    prelude::*,
    time::Duration,
};
use core::num::NonZeroUsize;
use wyhash::WyHash;

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    timer1: Timer<System>,
    timer2: Timer<System>,
    timer3: Timer<System>,
    timer4: Timer<System>,
    task: Task<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        let timer1 = Timer::build()
            .active(true)
            .delay(Duration::from_millis(200))
            .start(timer1_body::<System, D>)
            .param(42)
            .finish(b);

        let timer2 = Timer::build()
            .active(true)
            .delay(Duration::from_millis(100))
            .start(timer2_body::<System, D>)
            .param(52)
            .finish(b);

        let timer3 = Timer::build()
            .period(Duration::from_millis(0))
            .start(unreachable_timer_body::<System, D>)
            .finish(b);

        let timer4 = Timer::build()
            .delay(Duration::from_millis(0))
            .period(Duration::from_millis(0))
            .start(unreachable_timer_body::<System, D>)
            .finish(b);

        let task = Task::build()
            .active(true)
            .start(task_body::<System, D>)
            .priority(1)
            .finish(b);

        let seq = Hunk::<_, SeqTracker>::build().finish(b);

        App {
            timer1,
            timer2,
            timer3,
            timer4,
            task,
            seq,
        }
    }
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let App {
        seq,
        timer2,
        timer3,
        timer4,
        ..
    } = D::app();

    // Start `timer3`. `timer3` is now in the Active state, but it will never
    // fire because its delay is `None` (infinity).
    timer3.start().unwrap();

    // The same goes for `timer4`.
    timer4.set_delay(None).unwrap();
    timer4.start().unwrap();

    // `timer2` is already active, so this is no-op
    timer2.start().unwrap();

    // `timer2` wake-up time
    System::park().unwrap();
    seq.expect_and_replace(1, 2);

    #[cfg(feature = "system_time")]
    {
        let now = Time::from_millis(100);
        let now_got = System::time().unwrap();
        log::trace!("time = {:?} (expected {:?})", now_got, now);
        assert!(now_got.as_micros() >= now.as_micros());
        assert!(now_got.as_micros() <= now.as_micros() + 100_000);
    }

    // `timer1` wake-up time
    System::park().unwrap();
    seq.expect_and_replace(3, 4);

    #[cfg(feature = "system_time")]
    {
        let now = Time::from_millis(200);
        let now_got = System::time().unwrap();
        log::trace!("time = {:?} (expected {:?})", now_got, now);
        assert!(now_got.as_micros() >= now.as_micros());
        assert!(now_got.as_micros() <= now.as_micros() + 100_000);
    }

    D::success();
}

fn timer1_body<System: Kernel, D: Driver<App<System>>>(param: usize) {
    let App {
        timer1,
        timer2,
        task,
        seq,
        ..
    } = D::app();

    assert_eq!(param, 42);

    assert!(!System::is_task_context());

    // Check `timer1`'s expiration time in `task`
    // (`System::time` is disallowed in a non-task context)
    seq.expect_and_replace(2, 3);
    task.unpark().unwrap();

    // `PartialEq`
    assert_ne!(timer1, timer2);
    assert_eq!(timer1, timer1);
    assert_eq!(timer2, timer2);

    // `Hash`
    let hash = |x: &Timer<System>| {
        use core::hash::{Hash, Hasher};
        let mut hasher = WyHash::with_seed(42);
        x.hash(&mut hasher);
        hasher.finish()
    };
    assert_eq!(hash(timer1), hash(timer1));
    assert_eq!(hash(timer2), hash(timer2));

    // Disallowed in a non-task context
    #[cfg(feature = "priority_boost")]
    assert_eq!(
        System::boost_priority(),
        Err(kernel::BoostPriorityError::BadContext),
    );
    assert_eq!(
        unsafe { System::exit_task() },
        Err(kernel::ExitTaskError::BadContext),
    );
    assert_eq!(System::park(), Err(kernel::ParkError::BadContext));

    // Invalid ID
    let bad_timer: Timer<System> = unsafe { Timer::from_id(NonZeroUsize::new(42).unwrap()) };
    assert_eq!(
        bad_timer.start(),
        Err(r3::kernel::StartTimerError::BadId)
    );

    // Disallowed with CPU Lock acitve
    System::acquire_cpu_lock().unwrap();
    assert_eq!(
        timer1.start(),
        Err(r3::kernel::StartTimerError::BadContext)
    );
    assert_eq!(
        timer1.stop(),
        Err(r3::kernel::StopTimerError::BadContext)
    );
    assert_eq!(
        timer1.set_delay(None),
        Err(r3::kernel::SetTimerDelayError::BadContext)
    );
    assert_eq!(
        timer1.set_period(None),
        Err(r3::kernel::SetTimerPeriodError::BadContext)
    );
    unsafe { System::release_cpu_lock().unwrap() };

    // Negative duration
    assert_eq!(
        timer1.set_delay(Some(Duration::from_micros(-1))),
        Err(r3::kernel::SetTimerDelayError::BadParam)
    );
    assert_eq!(
        timer1.set_delay(Some(Duration::MIN)),
        Err(r3::kernel::SetTimerDelayError::BadParam)
    );
    assert_eq!(
        timer1.set_period(Some(Duration::from_micros(-1))),
        Err(r3::kernel::SetTimerPeriodError::BadParam)
    );
    assert_eq!(
        timer1.set_period(Some(Duration::MIN)),
        Err(r3::kernel::SetTimerPeriodError::BadParam)
    );
}

fn timer2_body<System: Kernel, D: Driver<App<System>>>(param: usize) {
    let App { task, seq, .. } = D::app();

    assert_eq!(param, 52);

    // Check `timer2`'s expiration time in `task`
    // (`System::time` is disallowed in a non-task context)
    seq.expect_and_replace(0, 1);
    task.unpark().unwrap();
}

fn unreachable_timer_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    unreachable!()
}
