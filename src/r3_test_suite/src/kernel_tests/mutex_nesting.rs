//! Locks multiple mutexes and makes sure the current task's effective priority
//! is updated correctly.
use r3::kernel::{prelude::*, traits, Cfg, LocalTask, MutexProtocol, StaticMutex, StaticTask};

use super::Driver;

pub trait SupportedSystem:
    traits::KernelBase + traits::KernelTaskSetPriority + traits::KernelMutex
{
}
impl<T: traits::KernelBase + traits::KernelTaskSetPriority + traits::KernelMutex> SupportedSystem
    for T
{
}

pub struct App<System: SupportedSystem> {
    m: [StaticMutex<System>; 4],
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System>
            + ~const traits::CfgTask
            + ~const traits::CfgMutex,
    {
        b.num_task_priority_levels(16);

        StaticTask::define()
            .start(task1_body::<System, D>)
            .priority(15)
            .active(true)
            .finish(b);

        let m = [
            StaticMutex::define()
                .protocol(MutexProtocol::Ceiling(0))
                .finish(b),
            StaticMutex::define()
                .protocol(MutexProtocol::Ceiling(1))
                .finish(b),
            StaticMutex::define()
                .protocol(MutexProtocol::Ceiling(2))
                .finish(b),
            StaticMutex::define()
                .protocol(MutexProtocol::Ceiling(3))
                .finish(b),
        ];

        App { m }
    }
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    let App { m } = D::app();

    let cur_task: LocalTask<System> = LocalTask::current().unwrap().unwrap();
    assert_eq!(cur_task.priority().unwrap(), 15);
    assert_eq!(cur_task.effective_priority().unwrap(), 15);

    m[3].lock().unwrap();
    assert_eq!(cur_task.priority().unwrap(), 15);
    assert_eq!(cur_task.effective_priority().unwrap(), 3);

    m[2].lock().unwrap();
    assert_eq!(cur_task.priority().unwrap(), 15);
    assert_eq!(cur_task.effective_priority().unwrap(), 2);

    m[1].lock().unwrap();
    assert_eq!(cur_task.priority().unwrap(), 15);
    assert_eq!(cur_task.effective_priority().unwrap(), 1);

    m[0].lock().unwrap();
    assert_eq!(cur_task.priority().unwrap(), 15);
    assert_eq!(cur_task.effective_priority().unwrap(), 0);

    cur_task.set_priority(12).unwrap();

    m[0].unlock().unwrap();
    assert_eq!(cur_task.priority().unwrap(), 12);
    assert_eq!(cur_task.effective_priority().unwrap(), 1);

    m[1].unlock().unwrap();
    assert_eq!(cur_task.priority().unwrap(), 12);
    assert_eq!(cur_task.effective_priority().unwrap(), 2);

    m[2].unlock().unwrap();
    assert_eq!(cur_task.priority().unwrap(), 12);
    assert_eq!(cur_task.effective_priority().unwrap(), 3);

    m[3].unlock().unwrap();
    assert_eq!(cur_task.priority().unwrap(), 12);
    assert_eq!(cur_task.effective_priority().unwrap(), 12);

    D::success();
}
