//! Activates and deactivates CPU Lock.
use r3::{
    kernel::{cfg::CfgBuilder, Task},
    prelude::*,
};
use core::marker::PhantomData;

use super::Driver;

pub struct App<System> {
    _phantom: PhantomData<System>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        Task::build()
            .start(task_body::<System, D>)
            .priority(0)
            .active(true)
            .finish(b);

        App {
            _phantom: PhantomData,
        }
    }
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    assert!(!System::has_cpu_lock());

    // Acquire CPU Lock
    System::acquire_cpu_lock().unwrap();

    // Can't do it again because it's already acquired
    assert!(System::has_cpu_lock());
    assert_eq!(
        System::acquire_cpu_lock(),
        Err(r3::kernel::CpuLockError::BadContext),
    );
    assert!(System::has_cpu_lock());

    // Release CPU Lock
    unsafe { System::release_cpu_lock() }.unwrap();

    // Can't do it again because it's already released
    assert!(!System::has_cpu_lock());
    assert_eq!(
        unsafe { System::release_cpu_lock() },
        Err(r3::kernel::CpuLockError::BadContext),
    );
    assert!(!System::has_cpu_lock());

    D::success();
}
