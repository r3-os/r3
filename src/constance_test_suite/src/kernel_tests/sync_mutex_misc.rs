//! Checks miscellaneous properties of [`constance::sync::Mutex`].
use assert_matches::assert_matches;
use constance::{
    kernel::{cfg::CfgBuilder, Hunk, InterruptHandler, InterruptLine, Task},
    prelude::*,
    sync::mutex::{self, Mutex},
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    int: Option<InterruptLine<System>>,
    eg1: Mutex<System, u32>,
    eg2: Mutex<System, u32>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        Task::build()
            .start(task_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);
        let eg1 = Mutex::build().finish(b);
        let eg2 = Mutex::build().finish(b);

        let int = if let (&[int_line, ..], &[int_pri, ..]) =
            (D::INTERRUPT_LINES, D::INTERRUPT_PRIORITIES)
        {
            InterruptHandler::build()
                .line(int_line)
                .start(isr::<System, D>)
                .finish(b);

            Some(
                InterruptLine::build()
                    .line(int_line)
                    .enabled(true)
                    .priority(int_pri)
                    .finish(b),
            )
        } else {
            None
        };

        let seq = Hunk::<_, SeqTracker>::build().finish(b);

        App { eg1, eg2, int, seq }
    }
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
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
    assert_matches!(app.eg1.lock(), Err(mutex::LockError::BadContext));
    assert_matches!(app.eg1.try_lock(), Err(mutex::TryLockError::BadContext));
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
        assert_matches!(app.eg1.try_lock(), Err(mutex::TryLockError::WouldBlock));
        assert_matches!(app.eg1.try_lock(), Err(mutex::TryLockError::WouldBlock));
    }

    // Inner data
    *app.eg1.lock().unwrap() = 0x12345678;
    *app.eg1.try_lock().unwrap() += 1;
    *app.eg2.lock().unwrap() = 0x87654321;
    *app.eg2.try_lock().unwrap() -= 1;

    assert_eq!(*app.eg1.lock().unwrap(), 0x12345679);
    assert_eq!(*app.eg2.lock().unwrap(), 0x87654320);

    assert_eq!(unsafe { *app.eg1.get_ptr() }, 0x12345679);
    assert_eq!(unsafe { *app.eg2.get_ptr() }, 0x87654320);

    D::success();
}

fn isr<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let app = D::app();

    app.seq.expect_and_replace(1, 2);

    // Non-task context
    assert_matches!(app.eg1.lock(), Err(mutex::LockError::BadContext));
    assert_matches!(app.eg1.try_lock(), Err(mutex::TryLockError::BadContext));
}
