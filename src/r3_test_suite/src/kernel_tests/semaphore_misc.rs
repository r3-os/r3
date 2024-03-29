//! Validates error codes returned by semaphore manipulation methods. Also,
//! checks miscellaneous properties of `Semaphore`.
use r3::kernel::{prelude::*, traits, Cfg, SemaphoreRef, StaticSemaphore, StaticTask};
use wyhash::WyHash;

use super::Driver;

pub trait SupportedSystem: traits::KernelBase + traits::KernelSemaphore {}
impl<T: traits::KernelBase + traits::KernelSemaphore> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    eg1: StaticSemaphore<System>,
    eg2: StaticSemaphore<System>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self, System = System>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgTask<System = System> + ~const traits::CfgSemaphore,
    {
        StaticTask::define()
            .start(task_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        let eg1 = StaticSemaphore::define().maximum(1).initial(1).finish(b);
        let eg2 = StaticSemaphore::define().maximum(2).initial(1).finish(b);

        App { eg1, eg2 }
    }
}

fn task_body<System: SupportedSystem, D: Driver<App<System>, System = System>>() {
    // `PartialEq`
    let app = D::app();
    assert_ne!(app.eg1, app.eg2);
    assert_eq!(app.eg1, app.eg1);
    assert_eq!(app.eg2, app.eg2);

    // `Hash`
    let hash = |x: SemaphoreRef<'_, System>| {
        use core::hash::{Hash, Hasher};
        let mut hasher = WyHash::with_seed(42);
        x.hash(&mut hasher);
        hasher.finish()
    };
    assert_eq!(hash(app.eg1), hash(app.eg1));
    assert_eq!(hash(app.eg2), hash(app.eg2));

    // Invalid semaphore ID
    if let Some(bad_id) = D::bad_raw_semaphore_id() {
        let bad_eg: SemaphoreRef<'_, System> = unsafe { SemaphoreRef::from_id(bad_id) };
        assert_eq!(bad_eg.get(), Err(r3::kernel::GetSemaphoreError::NoAccess));
    }

    // CPU Lock active
    System::acquire_cpu_lock().unwrap();
    assert_eq!(
        app.eg1.get(),
        Err(r3::kernel::GetSemaphoreError::BadContext)
    );
    assert_eq!(
        app.eg1.drain(),
        Err(r3::kernel::DrainSemaphoreError::BadContext)
    );
    assert_eq!(
        app.eg1.signal(1),
        Err(r3::kernel::SignalSemaphoreError::BadContext)
    );
    assert_eq!(
        app.eg1.wait_one(),
        Err(r3::kernel::WaitSemaphoreError::BadContext)
    );
    assert_eq!(
        app.eg1.poll_one(),
        Err(r3::kernel::PollSemaphoreError::BadContext)
    );
    unsafe { System::release_cpu_lock().unwrap() };

    // 1 (current) + 2 > 2 (maximum)
    assert_eq!(app.eg2.get().unwrap(), 1);
    assert_eq!(
        app.eg2.signal(2),
        Err(r3::kernel::SignalSemaphoreError::QueueOverflow)
    );

    // 1 (current) + 1 <= 2 (maximum)
    assert_eq!(app.eg2.get().unwrap(), 1);
    app.eg2.signal_one().unwrap();

    // 2 (current) + 1 > 2 (maximum)
    assert_eq!(app.eg2.get().unwrap(), 2);
    assert_eq!(
        app.eg2.signal(1),
        Err(r3::kernel::SignalSemaphoreError::QueueOverflow)
    );
    assert_eq!(
        app.eg2.signal(r3::kernel::SemaphoreValue::MAX),
        Err(r3::kernel::SignalSemaphoreError::QueueOverflow)
    );

    // 2 (current) + 0 <= 2 (maximum)
    assert_eq!(app.eg2.get().unwrap(), 2);
    app.eg2.signal(0).unwrap();

    // 2 (current) - 1 >= 0 (minimum)
    assert_eq!(app.eg2.get().unwrap(), 2);
    app.eg2.poll_one().unwrap();

    // 1 (current) - 1 >= 0 (minimum)
    assert_eq!(app.eg2.get().unwrap(), 1);
    app.eg2.poll_one().unwrap();

    // 0 (current) - 1 < 0 (minimum)
    assert_eq!(app.eg2.get().unwrap(), 0);
    assert_eq!(
        app.eg2.poll_one(),
        Err(r3::kernel::PollSemaphoreError::Timeout)
    );

    assert_eq!(app.eg2.get().unwrap(), 0);

    // (0 (current) + 2) * 0 (drain) = 0
    assert_eq!(app.eg2.get().unwrap(), 0);
    app.eg2.signal(2).unwrap();
    assert_eq!(app.eg2.get().unwrap(), 2);
    app.eg2.drain().unwrap();
    assert_eq!(app.eg2.get().unwrap(), 0);

    D::success();
}
