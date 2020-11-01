//! Validates error codes returned by event group manipulation methods. Also,
//! checks miscellaneous properties of `EventGroup`.
use r3::{
    kernel::{cfg::CfgBuilder, EventGroup, Task},
    prelude::*,
};
use core::num::NonZeroUsize;
use wyhash::WyHash;

use super::Driver;

pub struct App<System> {
    eg1: EventGroup<System>,
    eg2: EventGroup<System>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        Task::build()
            .start(task_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        let eg1 = EventGroup::build().finish(b);
        let eg2 = EventGroup::build().finish(b);

        App { eg1, eg2 }
    }
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
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
    assert_eq!(
        bad_eg.get(),
        Err(r3::kernel::GetEventGroupError::BadId)
    );

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
        app.eg1
            .wait(0, r3::kernel::EventGroupWaitFlags::empty()),
        Err(r3::kernel::WaitEventGroupError::BadContext)
    );
    unsafe { System::release_cpu_lock().unwrap() };

    D::success();
}
