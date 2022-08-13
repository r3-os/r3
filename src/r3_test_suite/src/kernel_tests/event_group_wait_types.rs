//! Waits for an event group with various wait flags.
use r3::kernel::{prelude::*, traits, Cfg, EventGroupWaitFlags, StaticEventGroup, StaticTask};

use super::Driver;

pub trait SupportedSystem: traits::KernelBase + traits::KernelEventGroup {}
impl<T: traits::KernelBase + traits::KernelEventGroup> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    eg: StaticEventGroup<System>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgTask<System = System> + ~const traits::CfgEventGroup,
    {
        StaticTask::define()
            .start(task1_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);

        let eg = StaticEventGroup::define().finish(b);

        App { eg }
    }
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>() {
    let eg = D::app().eg;

    eg.set(0b100011).unwrap();
    eg.clear(0b100000).unwrap();
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
