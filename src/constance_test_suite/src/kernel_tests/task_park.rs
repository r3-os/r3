//! Sequence the execution of tasks using the parking mechanism.
use constance::{
    kernel::{Hunk, Task},
    prelude::*,
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    task2: Task<System>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            new! { Task<_>, start = task1_body::<System, D>, priority = 2, active = true };
            let task2 = new! { Task<_>, start = task2_body::<System, D>, priority = 1, active = true };

            let seq = new! { Hunk<_, SeqTracker> };

            App { task2, seq }
        }
    }
}

fn task1_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(1, 2);

    D::app().task2.unpark_exact().unwrap();

    D::app().seq.expect_and_replace(3, 4);

    D::app().task2.interrupt().unwrap();
}

fn task2_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(0, 1);

    System::park().unwrap(); // blocks, switching to `task1`

    D::app().seq.expect_and_replace(2, 3);

    assert_eq!(
        // blocks, switching to `task1`
        System::park(),
        Err(constance::kernel::ParkError::Interrupted)
    );

    D::app().seq.expect_and_replace(4, 5);

    // Give a park token to itself
    D::app().task2.unpark_exact().unwrap();
    // `park` doesn't block if the task already has a token
    System::park().unwrap();

    D::app().task2.unpark_exact().unwrap();
    assert_eq!(
        D::app().task2.unpark_exact(),
        Err(constance::kernel::UnparkExactError::QueueOverflow)
    );

    D::success();
}
