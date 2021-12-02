//! Waits for an event group with various wait flags.
use r3::kernel::{traits, Cfg, EventGroup, EventGroupWaitFlags, Task};

use super::Driver;

pub trait SupportedSystem: traits::KernelBase + traits::KernelEventGroup {}
impl<T: traits::KernelBase + traits::KernelEventGroup> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    eg: EventGroup<System>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System>
            + ~const traits::CfgTask
            + ~const traits::CfgEventGroup,
    {
        Task::build()
            .start(task1_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);

        let eg = EventGroup::build().finish(b);

        App { eg }
    }
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
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
