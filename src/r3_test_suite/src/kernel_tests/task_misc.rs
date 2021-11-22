//! Validates error codes returned by task manipulation methods. Also, checks
//! miscellaneous properties of `Task`.
use core::num::NonZeroUsize;
use r3::kernel::{prelude::*, traits, Cfg, StartupHook, Task};
use wyhash::WyHash;

use super::Driver;

// TODO: Somehow remove the `NonZeroUsize` bound
pub trait SupportedSystem:
    traits::KernelBase<TaskId = NonZeroUsize> + traits::KernelTaskSetPriority
{
}
impl<T: traits::KernelBase<TaskId = NonZeroUsize> + traits::KernelTaskSetPriority> SupportedSystem
    for T
{
}

pub struct App<System: SupportedSystem> {
    task1: Task<System>,
    task2: Task<System>,
    task3: Task<System>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System> + ~const traits::CfgTask,
    {
        StartupHook::build()
            .start(startup_hook::<System, D>)
            .finish(b);

        let task1 = Task::build()
            .start(task1_body::<System, D>)
            .priority(2)
            .active(true)
            .param(42)
            .finish(b);
        let task2 = Task::build()
            .start(task2_body::<System, D>)
            .priority(1)
            .finish(b);
        let task3 = Task::build()
            .start(task3_body::<System, D>)
            .priority(1)
            .finish(b);

        App {
            task1,
            task2,
            task3,
        }
    }
}

fn startup_hook<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    assert_eq!(
        Task::<System>::current(),
        Err(r3::kernel::GetCurrentTaskError::BadContext)
    );
}

fn task1_body<System: SupportedSystem, D: Driver<App<System>>>(param: usize) {
    assert_eq!(param, 42);

    // `PartialEq`
    let app = D::app();
    assert_ne!(app.task1, app.task2);
    assert_eq!(app.task1, app.task1);
    assert_eq!(app.task2, app.task2);

    // `Hash`
    let hash = |x: Task<System>| {
        use core::hash::{Hash, Hasher};
        let mut hasher = WyHash::with_seed(42);
        x.hash(&mut hasher);
        hasher.finish()
    };
    assert_eq!(hash(app.task1), hash(app.task1));
    assert_eq!(hash(app.task2), hash(app.task2));

    // Invalid task ID
    let bad_task: Task<System> = unsafe { Task::from_id(NonZeroUsize::new(42).unwrap()) };
    assert_eq!(
        bad_task.activate(),
        Err(r3::kernel::ActivateTaskError::BadId)
    );

    // The task is already active
    assert_eq!(
        app.task1.activate(),
        Err(r3::kernel::ActivateTaskError::QueueOverflow)
    );

    // The task is dormant
    assert_eq!(
        app.task2.interrupt(),
        Err(r3::kernel::InterruptTaskError::BadObjectState)
    );
    assert_eq!(
        app.task2.set_priority(1),
        Err(r3::kernel::SetTaskPriorityError::BadObjectState)
    );
    assert_eq!(
        app.task2.priority(),
        Err(r3::kernel::GetTaskPriorityError::BadObjectState)
    );
    assert_eq!(
        app.task2.effective_priority(),
        Err(r3::kernel::GetTaskPriorityError::BadObjectState)
    );

    // The task is running
    assert_eq!(
        app.task1.interrupt(),
        Err(r3::kernel::InterruptTaskError::BadObjectState)
    );

    assert_eq!(app.task1.priority(), Ok(2));
    assert_eq!(app.task1.effective_priority(), Ok(2));

    // Priority is out of range
    assert_eq!(
        app.task1.set_priority(usize::MAX),
        Err(r3::kernel::SetTaskPriorityError::BadParam)
    );

    assert_eq!(app.task1.priority(), Ok(2));
    assert_eq!(app.task1.effective_priority(), Ok(2));

    // Current task
    // This assertion might not be useful because `task1` always has ID 0, so
    // it's unlikely to catch errors such as dividing a pointer difference by a
    // wrong divisor. For this reason, we check this again in a different
    // task.
    assert_eq!(Task::current().unwrap(), Some(app.task1));

    // CPU Lock active
    System::acquire_cpu_lock().unwrap();
    assert_eq!(
        app.task1.activate(),
        Err(r3::kernel::ActivateTaskError::BadContext)
    );
    assert_eq!(
        app.task1.interrupt(),
        Err(r3::kernel::InterruptTaskError::BadContext)
    );
    assert_eq!(app.task1.unpark(), Err(r3::kernel::UnparkError::BadContext));
    assert_eq!(System::park(), Err(r3::kernel::ParkError::BadContext));
    assert_eq!(
        app.task1.set_priority(2),
        Err(r3::kernel::SetTaskPriorityError::BadContext)
    );
    assert_eq!(
        app.task1.priority(),
        Err(r3::kernel::GetTaskPriorityError::BadContext)
    );
    assert_eq!(
        app.task1.effective_priority(),
        Err(r3::kernel::GetTaskPriorityError::BadContext)
    );
    unsafe { System::release_cpu_lock().unwrap() };

    // Go to `task3_body`
    app.task3.activate().unwrap();
}

fn task2_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    unreachable!();
}

fn task3_body<System: SupportedSystem, D: Driver<App<System>>>(_: usize) {
    // Current task (again)
    assert_eq!(Task::current().unwrap(), Some(D::app().task3));

    D::success();
}
