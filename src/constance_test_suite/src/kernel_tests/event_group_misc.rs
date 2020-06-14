//! Validates error codes returned by event group manipulation methods. Also,
//! checks miscellaneous properties of `EventGroup`.
use constance::{
    kernel::{EventGroup, Task},
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
    constance::configure! {
        pub fn new<D: Driver<Self>>(_: CfgBuilder<System>) -> Self {
            build! {
                Task<_>,
                start = task_body::<System, D>,
                priority = 2,
                active = true,
            };
            let eg1 = build! { EventGroup<_> };
            let eg2 = build! { EventGroup<_> };

            App { eg1, eg2 }
        }
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
        Err(constance::kernel::GetEventGroupError::BadId)
    );

    D::success();
}
