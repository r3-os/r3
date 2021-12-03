//! Validates error codes returned by event group manipulation methods. Also,
//! checks miscellaneous properties of `EventGroup`.
use core::num::NonZeroUsize;
use r3::kernel::{prelude::*, traits, Cfg, EventGroup, Task};
use wyhash::WyHash;

use super::Driver;

// TODO: Somehow remove the `NonZeroUsize` bound
pub trait SupportedSystem:
    traits::KernelBase + traits::KernelEventGroup<RawEventGroupId = NonZeroUsize>
{
}
impl<T: traits::KernelBase + traits::KernelEventGroup<RawEventGroupId = NonZeroUsize>>
    SupportedSystem for T
{
}

pub struct App<System: SupportedSystem> {
    eg1: EventGroup<System>,
    eg2: EventGroup<System>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System>
            + ~const traits::CfgTask
            + ~const traits::CfgEventGroup,
    {
        Task::define()
            .start(task_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        let eg1 = EventGroup::define().finish(b);
        let eg2 = EventGroup::define().finish(b);

        App { eg1, eg2 }
    }
}

fn task_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    // `PartialEq`
    let app = D::app();
    assert_ne!(app.eg1, app.eg2);
    assert_eq!(app.eg1, app.eg1);
    assert_eq!(app.eg2, app.eg2);

    // `Hash`
    let hash = |x: EventGroup<System>| {
        use core::hash::{Hash, Hasher};
        let mut hasher = WyHash::with_seed(42);
        x.hash(&mut hasher);
        hasher.finish()
    };
    assert_eq!(hash(app.eg1), hash(app.eg1));
    assert_eq!(hash(app.eg2), hash(app.eg2));

    // Invalid event group ID
    let bad_eg: EventGroup<System> = unsafe { EventGroup::from_id(NonZeroUsize::new(42).unwrap()) };
    assert_eq!(bad_eg.get(), Err(r3::kernel::GetEventGroupError::BadId));

    // CPU Lock active
    System::acquire_cpu_lock().unwrap();
    assert_eq!(
        app.eg1.get(),
        Err(r3::kernel::GetEventGroupError::BadContext)
    );
    assert_eq!(
        app.eg1.set(0),
        Err(r3::kernel::UpdateEventGroupError::BadContext)
    );
    assert_eq!(
        app.eg1.clear(0),
        Err(r3::kernel::UpdateEventGroupError::BadContext)
    );
    assert_eq!(
        app.eg1.wait(0, r3::kernel::EventGroupWaitFlags::empty()),
        Err(r3::kernel::WaitEventGroupError::BadContext)
    );
    unsafe { System::release_cpu_lock().unwrap() };

    D::success();
}
