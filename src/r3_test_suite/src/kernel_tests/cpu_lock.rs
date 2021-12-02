//! Activates and deactivates CPU Lock.
use core::marker::PhantomData;
use r3::kernel::{prelude::*, traits, Cfg, Task};

use super::Driver;

pub struct App<System> {
    _phantom: PhantomData<System>,
}

impl<System: traits::KernelBase> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System> + ~const traits::CfgTask,
    {
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

fn task_body<System: traits::KernelBase, D: Driver<App<System>>>(_: usize) {
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
