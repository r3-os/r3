//! Validates error codes returned by time-related methods. Also, checks
//! miscellaneous properties of such methods.
use core::marker::PhantomData;
use r3::{
    kernel::{prelude::*, traits, Cfg, StartupHook, StaticTask},
    time::{Duration, Time},
};

use super::Driver;
use crate::utils::conditional::KernelTimeExt;

pub trait SupportedSystem: traits::KernelBase + traits::KernelAdjustTime + KernelTimeExt {}
impl<T: traits::KernelBase + traits::KernelAdjustTime + KernelTimeExt> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    _phantom: PhantomData<System>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System> + ~const traits::CfgTask,
    {
        StartupHook::define()
            .start(startup_hook::<System, D>)
            .finish(b);
        StaticTask::define()
            .start(task_body::<System, D>)
            .priority(0)
            .active(true)
            .finish(b);

        App {
            _phantom: PhantomData,
        }
    }
}

fn startup_hook<System: SupportedSystem, D: Driver<App<System>>>() {
    // Not a task context
    if let Some(cap) = System::TIME_CAPABILITY {
        assert_eq!(System::time(cap), Err(r3::kernel::TimeError::BadContext));
    }

    assert_eq!(
        System::set_time(Time::from_micros(0)),
        Err(r3::kernel::TimeError::BadContext)
    );
    assert_eq!(
        System::adjust_time(Duration::ZERO),
        Err(r3::kernel::AdjustTimeError::BadContext)
    );
    assert_eq!(
        System::sleep(Duration::from_micros(0)),
        Err(r3::kernel::SleepError::BadContext)
    );
}

fn task_body<System: SupportedSystem, D: Driver<App<System>>>() {
    let now = if let Some(cap) = System::TIME_CAPABILITY {
        let now = System::time(cap).unwrap();
        log::trace!("time = {:?}", now);
        Some((cap, now))
    } else {
        None
    };

    if let Some((cap, now)) = now {
        // Because this task is activated at boot, the current time should be
        // very close to zero
        assert_eq!(now.as_secs(), 0);

        // Now change the time
        let now2 = Time::from_millis(114514);
        log::trace!("changing system time to {:?}", now2);
        System::set_time(now2).unwrap();

        // Because we just changed the time to `now2`, the current time should be
        // still very close to `now2`
        let now2_got = System::time(cap).unwrap();
        log::trace!("time = {:?}", now2_got);
        assert_eq!(now2_got.duration_since(now2).unwrap().as_secs(), 0);
    }

    // CPU Lock active
    System::acquire_cpu_lock().unwrap();
    if let Some(cap) = System::TIME_CAPABILITY {
        assert_eq!(System::time(cap), Err(r3::kernel::TimeError::BadContext));
    }

    assert_eq!(
        System::set_time(Time::from_millis(0)),
        Err(r3::kernel::TimeError::BadContext)
    );
    assert_eq!(
        System::adjust_time(Duration::ZERO),
        Err(r3::kernel::AdjustTimeError::BadContext)
    );
    assert_eq!(
        System::sleep(Duration::from_micros(0)),
        Err(r3::kernel::SleepError::BadContext)
    );
    unsafe { System::release_cpu_lock().unwrap() };

    let now4_got = if let Some(cap) = System::TIME_CAPABILITY {
        // System time should wrap around
        let now3 = Time::from_micros(0xfffffffffffe0000);
        log::trace!("changing system time to {:?}", now3);
        System::set_time(now3).unwrap();

        let d = Duration::from_micros(0x40000);
        log::trace!("sleeping for {:?}", d);
        System::sleep(d).unwrap();

        let now4 = now3 + d;
        let now4_got = System::time(cap).unwrap();
        log::trace!("time = {:?} (expected >= {:?})", now4_got, now4);
        assert!(now4_got.as_micros() >= now4.as_micros());

        Some((cap, now4_got))
    } else {
        None
    };

    // `adjust_time(0)` is no-op
    System::adjust_time(Duration::ZERO).unwrap();

    if let Some((cap, now4_got)) = now4_got {
        let now5 = now4_got;
        let now5_got = System::time(cap).unwrap();
        log::trace!("time = {:?} (expected {:?})", now5_got, now5);
        assert!(now5_got.as_micros() >= now5.as_micros());
        assert!(now5_got.as_micros() <= now5.as_micros() + 100_000);
    }

    // Out-of-range duration
    assert_eq!(
        System::sleep(Duration::from_micros(-1)),
        Err(r3::kernel::SleepError::BadParam)
    );
    assert_eq!(
        System::sleep(Duration::MIN),
        Err(r3::kernel::SleepError::BadParam)
    );

    // Wait zero microseconds
    log::trace!("sleep(0)");
    System::sleep(Duration::ZERO).unwrap();

    D::success();
}
