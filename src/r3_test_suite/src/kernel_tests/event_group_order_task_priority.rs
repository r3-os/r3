//! Sets an event group, waking up multiple tasks in a task priority order.
//!
//! 1. (`seq`: 0 → 1) `task0` activates `task[1-4]` in a particular order.
//! 2. (`seq`: 1 → 5) `task[1-4]` start waiting for a event bit to be set.
//! 3. (`seq`: 5 → 9) `task0` sets the event bit for four times. `task[1-4]`
//!    should be unblocked in a task priority order.
//!
use r3::{
    hunk::Hunk,
    kernel::{traits, Cfg, EventGroup, EventGroupWaitFlags, QueueOrder, Task},
};

use super::Driver;
use crate::utils::SeqTracker;

pub trait SupportedSystem:
    traits::KernelBase + traits::KernelEventGroup + traits::KernelStatic
{
}
impl<T: traits::KernelBase + traits::KernelEventGroup + traits::KernelStatic> SupportedSystem
    for T
{
}

pub struct App<System: SupportedSystem> {
    eg: EventGroup<System>,
    task1: Task<System>,
    task2: Task<System>,
    task3: Task<System>,
    task4: Task<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System>
            + ~const traits::CfgTask
            + ~const traits::CfgEventGroup,
    {
        Task::define()
            .start(task0_body::<System, D>)
            .priority(3)
            .active(true)
            .finish(b);
        let task1 = Task::define()
            .start(task1_body::<System, D>)
            .priority(1)
            .finish(b);
        let task2 = Task::define()
            .start(task2_body::<System, D>)
            .priority(1)
            .finish(b);
        let task3 = Task::define()
            .start(task3_body::<System, D>)
            .priority(2)
            .finish(b);
        let task4 = Task::define()
            .start(task4_body::<System, D>)
            .priority(2)
            .finish(b);

        let eg = EventGroup::define()
            .queue_order(QueueOrder::TaskPriority)
            .finish(b);
        let seq = Hunk::<_, SeqTracker>::define().finish(b);

        App {
            eg,
            task1,
            task2,
            task3,
            task4,
            seq,
        }
    }
}

fn task0_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    D::app().task3.activate().unwrap(); // [3]
    D::app().task1.activate().unwrap(); // [1, 3]
    D::app().task2.activate().unwrap(); // [1, 2, 3]
    D::app().task4.activate().unwrap(); // [1, 2, 3, 4]

    D::app().eg.set(0b1).unwrap(); // unblocks `task1`
    D::app().eg.set(0b1).unwrap(); // unblocks `task2`
    D::app().eg.set(0b1).unwrap(); // unblocks `task3`
    D::app().eg.set(0b1).unwrap(); // unblocks `task4`

    D::app().seq.expect_and_replace(9, 10);
    D::success();
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(2, 3);

    D::app().eg.wait(0b1, EventGroupWaitFlags::CLEAR).unwrap(); // start waiting, switching to `task0`

    D::app().seq.expect_and_replace(5, 6);
    // return the control to `task0`
}

fn task2_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(3, 4);

    D::app().eg.wait(0b1, EventGroupWaitFlags::CLEAR).unwrap(); // start waiting, switching to `task0`

    D::app().seq.expect_and_replace(6, 7);
    // return the control to `task0`
}

fn task3_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(1, 2);

    D::app().eg.wait(0b1, EventGroupWaitFlags::CLEAR).unwrap(); // start waiting, switching to `task0`

    D::app().seq.expect_and_replace(7, 8);
    // return the control to `task0`
}

fn task4_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(4, 5);

    D::app().eg.wait(0b1, EventGroupWaitFlags::CLEAR).unwrap(); // start waiting, switching to `task0`

    D::app().seq.expect_and_replace(8, 9);
    // return the control to `task0`
}
