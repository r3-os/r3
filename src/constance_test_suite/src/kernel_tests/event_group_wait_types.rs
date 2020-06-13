//! Waits for an event group with various wait flags.
use constance::{
    kernel::{EventGroup, EventGroupWaitFlags},
    prelude::*,
};

use super::Driver;

pub struct App<System> {
    eg: EventGroup<System>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub fn new<D: Driver<Self>>(_: CfgBuilder<System>) -> Self {
            new_task! { start = task1_body::<System, D>, priority = 2, active = true };

            let eg = new_event_group! {};

            App { eg }
        }
    }
}

fn task1_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let eg = D::app().eg;

    eg.set(0b11).unwrap();
    eg.wait(0b11111, EventGroupWaitFlags::CLEAR).unwrap();
    assert_eq!(eg.get().unwrap(), 0b00);

    eg.set(0b11).unwrap();
    eg.wait(0b11111, EventGroupWaitFlags::empty()).unwrap();
    assert_eq!(eg.get().unwrap(), 0b11);

    eg.set(0b11).unwrap();
    eg.wait(0b1, EventGroupWaitFlags::ALL | EventGroupWaitFlags::CLEAR)
        .unwrap();
    assert_eq!(eg.get().unwrap(), 0b10);

    D::success();
}
