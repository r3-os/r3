//! Waits for an event group with various wait flags.
use constance::{
    kernel::{cfg::CfgBuilder, EventGroup, EventGroupWaitFlags, Task},
    prelude::*,
};

use super::Driver;

pub struct App<System> {
    eg: EventGroup<System>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        Task::build()
            .start(task1_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);

        let eg = EventGroup::build().finish(b);

        App { eg }
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
