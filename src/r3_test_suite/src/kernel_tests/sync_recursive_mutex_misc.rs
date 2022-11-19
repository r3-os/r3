//! Checks miscellaneous properties of [`r3::sync::StaticRecursiveMutex`].
use assert_matches::assert_matches;
use core::cell::Cell;
use r3::{
    hunk::Hunk,
    kernel::{prelude::*, traits, Cfg, InterruptLine, StaticInterruptHandler, StaticTask},
    sync::recursive_mutex::{self, StaticRecursiveMutex},
};

use super::Driver;
use crate::utils::SeqTracker;

pub trait SupportedSystem:
    traits::KernelBase + traits::KernelInterruptLine + traits::KernelMutex + traits::KernelStatic
{
}
impl<
        T: traits::KernelBase
            + traits::KernelInterruptLine
            + traits::KernelMutex
            + traits::KernelStatic,
    > SupportedSystem for T
{
}

pub struct App<System: SupportedSystem> {
    int: Option<InterruptLine<System>>,
    eg1: StaticRecursiveMutex<System, Cell<u32>>,
    eg2: StaticRecursiveMutex<System, Cell<u32>>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgTask<System = System>
            + ~const traits::CfgInterruptLine
            + ~const traits::CfgMutex,
    {
        StaticTask::define()
            .start(task_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        let eg1 = StaticRecursiveMutex::define().finish(b);
        let eg2 = StaticRecursiveMutex::define().finish(b);

        let int = if let (&[int_line, ..], &[int_pri, ..]) =
            (D::INTERRUPT_LINES, D::INTERRUPT_PRIORITIES)
        {
            StaticInterruptHandler::define()
                .line(int_line)
                .start(isr::<System, D>)
                .finish(b);

            Some(
                InterruptLine::define()
                    .line(int_line)
                    .enabled(true)
                    .priority(int_pri)
                    .finish(b),
            )
        } else {
            None
        };

        let seq = Hunk::<_, SeqTracker>::define().finish(b);

        App { eg1, eg2, int, seq }
    }
}

fn task_body<System: SupportedSystem, D: Driver<App<System>>>() {
    let app = D::app();

    app.seq.expect_and_replace(0, 1);

    if let Some(int) = app.int {
        int.pend().unwrap();
    } else {
        log::warn!("No interrupt lines defined, skipping a portion of the test");
        app.seq.expect_and_replace(1, 2);
    }

    // CPU Lock active
    System::acquire_cpu_lock().unwrap();
    assert_matches!(app.eg1.lock(), Err(recursive_mutex::LockError::BadContext));
    assert_matches!(
        app.eg1.try_lock(),
        Err(recursive_mutex::TryLockError::BadContext)
    );
    unsafe { System::release_cpu_lock().unwrap() };

    // Smoke test
    drop(app.eg1.lock());
    drop(app.eg1.lock());
    {
        let _eg1 = app.eg1.lock();
        drop(app.eg2.lock());
        drop(app.eg2.lock());
    }

    drop(app.eg1.try_lock());
    drop(app.eg1.try_lock());

    // Double lock
    {
        let _eg1 = app.eg1.lock();
        let _eg1b = app.eg1.try_lock();
        let _eg1c = app.eg1.try_lock();
    }

    // Inner data
    app.eg1.lock().unwrap().set(0x12345678);
    app.eg1.try_lock().unwrap().update(|x| x + 1);
    app.eg2.lock().unwrap().set(0x87654321);
    app.eg2.try_lock().unwrap().update(|x| x - 1);

    assert_eq!(app.eg1.lock().unwrap().get(), 0x12345679);
    assert_eq!(app.eg2.lock().unwrap().get(), 0x87654320);

    assert_eq!(unsafe { *app.eg1.get_ptr().cast::<u32>() }, 0x12345679);
    assert_eq!(unsafe { *app.eg2.get_ptr().cast::<u32>() }, 0x87654320);

    D::success();
}

fn isr<System: SupportedSystem, D: Driver<App<System>>>() {
    let app = D::app();

    app.seq.expect_and_replace(1, 2);

    // Non-task context
    assert_matches!(app.eg1.lock(), Err(recursive_mutex::LockError::BadContext));
    assert_matches!(
        app.eg1.try_lock(),
        Err(recursive_mutex::TryLockError::BadContext)
    );
}
