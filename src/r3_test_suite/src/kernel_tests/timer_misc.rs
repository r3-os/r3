//! Checks miscellaneous properties of `Timer`.
use r3::{
    hunk::Hunk,
    kernel::{self, prelude::*, traits, Cfg, StaticTask, StaticTimer, TimerRef},
    time::{Duration, Time},
};
use wyhash::WyHash;

use super::Driver;
use crate::utils::{
    conditional::{KernelBoostPriorityExt, KernelTimeExt},
    SeqTracker,
};

pub trait SupportedSystem:
    traits::KernelBase
    + traits::KernelTimer
    + traits::KernelStatic
    + KernelBoostPriorityExt
    + KernelTimeExt
{
}
impl<
        T: traits::KernelBase
            + traits::KernelTimer
            + traits::KernelStatic
            + KernelBoostPriorityExt
            + KernelTimeExt,
    > SupportedSystem for T
{
}

pub struct App<System: SupportedSystem> {
    timer1: StaticTimer<System>,
    timer2: StaticTimer<System>,
    timer3: StaticTimer<System>,
    timer4: StaticTimer<System>,
    task: StaticTask<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self, System = System>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgTask<System = System> + ~const traits::CfgTimer,
    {
        let timer1 = StaticTimer::define()
            .active(true)
            .delay(Duration::from_millis(200))
            .start((42, timer1_body::<System, D>))
            .finish(b);

        let timer2 = StaticTimer::define()
            .active(true)
            .delay(Duration::from_millis(100))
            .start((52, timer2_body::<System, D>))
            .finish(b);

        let timer3 = StaticTimer::define()
            .period(Duration::from_millis(0))
            .start(unreachable_timer_body)
            .finish(b);

        let timer4 = StaticTimer::define()
            .delay(Duration::from_millis(0))
            .period(Duration::from_millis(0))
            .start(unreachable_timer_body)
            .finish(b);

        let task = StaticTask::define()
            .active(true)
            .start(task_body::<System, D>)
            .priority(1)
            .finish(b);

        let seq = Hunk::<_, SeqTracker>::define().finish(b);

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

fn task_body<System: SupportedSystem, D: Driver<App<System>>>() {
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

    if let Some(cap) = System::TIME_CAPABILITY {
        let now = Time::from_millis(100);
        let now_got = System::time(cap).unwrap();
        log::trace!("time = {now_got:?} (expected {now:?})");
        assert!(now_got.as_micros() >= now.as_micros());
        assert!(now_got.as_micros() <= now.as_micros() + 100_000);
    }

    // `timer1` wake-up time
    System::park().unwrap();
    seq.expect_and_replace(3, 4);

    if let Some(cap) = System::TIME_CAPABILITY {
        let now = Time::from_millis(200);
        let now_got = System::time(cap).unwrap();
        log::trace!("time = {now_got:?} (expected {now:?})");
        assert!(now_got.as_micros() >= now.as_micros());
        assert!(now_got.as_micros() <= now.as_micros() + 100_000);
    }

    D::success();
}

fn timer1_body<System: SupportedSystem, D: Driver<App<System>, System = System>>(param: usize) {
    let App {
        timer1,
        timer2,
        task,
        seq,
        ..
    } = D::app();

    assert_eq!(param, 42);

    // Context query
    assert!(!System::is_task_context());
    assert!(System::is_interrupt_context());
    assert!(System::is_boot_complete());

    // Check `timer1`'s expiration time in `task`
    // (`System::time` is disallowed in a non-task context)
    seq.expect_and_replace(2, 3);
    task.unpark().unwrap();

    // `PartialEq`
    assert_ne!(timer1, timer2);
    assert_eq!(timer1, timer1);
    assert_eq!(timer2, timer2);

    // `Hash`
    let hash = |x: TimerRef<'_, System>| {
        use core::hash::{Hash, Hasher};
        let mut hasher = WyHash::with_seed(42);
        x.hash(&mut hasher);
        hasher.finish()
    };
    assert_eq!(hash(*timer1), hash(*timer1));
    assert_eq!(hash(*timer2), hash(*timer2));

    // Disallowed in a non-task context
    if let Some(cap) = System::BOOST_PRIORITY_CAPABILITY {
        assert_eq!(
            System::boost_priority(cap),
            Err(kernel::BoostPriorityError::BadContext),
        );
    }
    assert_eq!(
        unsafe { System::exit_task() },
        Err(kernel::ExitTaskError::BadContext),
    );
    assert_eq!(System::park(), Err(kernel::ParkError::BadContext));

    // Invalid ID
    if let Some(bad_id) = D::bad_raw_timer_id() {
        let bad_timer: TimerRef<'_, System> = unsafe { TimerRef::from_id(bad_id) };
        assert_eq!(
            bad_timer.start(),
            Err(r3::kernel::StartTimerError::NoAccess)
        );
    }

    // Disallowed with CPU Lock acitve
    System::acquire_cpu_lock().unwrap();
    assert_eq!(timer1.start(), Err(r3::kernel::StartTimerError::BadContext));
    assert_eq!(timer1.stop(), Err(r3::kernel::StopTimerError::BadContext));
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

fn timer2_body<System: SupportedSystem, D: Driver<App<System>>>(param: usize) {
    let App { task, seq, .. } = D::app();

    assert_eq!(param, 52);

    // Check `timer2`'s expiration time in `task`
    // (`System::time` is disallowed in a non-task context)
    seq.expect_and_replace(0, 1);
    task.unpark().unwrap();
}

fn unreachable_timer_body() {
    unreachable!()
}
