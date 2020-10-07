//! Validates error codes returned by mutex manipulation methods. Also,
//! checks miscellaneous properties of [`constance::kernel::Mutex`].
use constance::{
    kernel::{cfg::CfgBuilder, Hunk, InterruptHandler, InterruptLine, Mutex, Task},
    prelude::*,
    time::Duration,
};
use core::num::NonZeroUsize;
use staticvec::StaticVec;
use wyhash::WyHash;

use super::Driver;
use crate::utils::SeqTracker;

const N: usize = 4;

pub struct App<System> {
    int: Option<InterruptLine<System>>,
    m: [Mutex<System>; N],
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        Task::build()
            .start(task1_body::<System, D>)
            .priority(2)
            .active(true)
            .finish(b);

        Task::build()
            .start(task2_body::<System, D>)
            .priority(3)
            .active(true)
            .finish(b);

        let m = [
            Mutex::build().finish(b),
            Mutex::build().finish(b),
            Mutex::build().finish(b),
            Mutex::build().finish(b),
        ];

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

        App { int, m, seq }
    }
}

fn task1_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let app = D::app();

    app.seq.expect_and_replace(0, 1);

    if let Some(int) = app.int {
        int.pend().unwrap();
    } else {
        log::warn!("No interrupt lines defined, skipping a portion of the test");
        app.seq.expect_and_replace(1, 2);
    }

    // `PartialEq`
    let [m1, m2, ..] = app.m;
    assert_ne!(m1, m2);
    assert_eq!(m1, m1);
    assert_eq!(m2, m2);

    // `Hash`
    let hash = |x: Mutex<System>| {
        use core::hash::{Hash, Hasher};
        let mut hasher = WyHash::with_seed(42);
        x.hash(&mut hasher);
        hasher.finish()
    };
    assert_eq!(hash(m1), hash(m1));
    assert_eq!(hash(m2), hash(m2));

    // Invalid mutex ID
    let bad_m: Mutex<System> = unsafe { Mutex::from_id(NonZeroUsize::new(42).unwrap()) };
    assert_eq!(
        bad_m.is_locked(),
        Err(constance::kernel::QueryMutexError::BadId)
    );

    // CPU Lock active
    System::acquire_cpu_lock().unwrap();
    assert_eq!(
        m1.is_locked(),
        Err(constance::kernel::QueryMutexError::BadContext)
    );
    assert_eq!(
        m1.unlock(),
        Err(constance::kernel::UnlockMutexError::BadContext)
    );
    assert_eq!(
        m1.lock(),
        Err(constance::kernel::LockMutexError::BadContext)
    );
    assert_eq!(
        m1.try_lock(),
        Err(constance::kernel::TryLockMutexError::BadContext)
    );
    assert_eq!(
        m1.lock_timeout(Duration::ZERO),
        Err(constance::kernel::LockMutexTimeoutError::BadContext)
    );
    assert_eq!(
        m1.mark_consistent(),
        Err(constance::kernel::MarkConsistentMutexError::BadContext)
    );
    unsafe { System::release_cpu_lock().unwrap() };

    // Disallowed in a task, non-waitable context
    System::boost_priority().unwrap();
    assert_eq!(
        m1.unlock(),
        Err(constance::kernel::UnlockMutexError::BadContext)
    );
    assert_eq!(
        m1.lock(),
        Err(constance::kernel::LockMutexError::BadContext)
    );
    assert_eq!(
        m1.lock_timeout(Duration::ZERO),
        Err(constance::kernel::LockMutexTimeoutError::BadContext)
    );

    // Allowed in a task, non-waitable context
    m1.try_lock().unwrap();
    assert_eq!(m1.is_locked(), Ok(true));
    assert_eq!(
        m1.mark_consistent(),
        Err(constance::kernel::MarkConsistentMutexError::BadObjectState)
    );
    unsafe { System::unboost_priority().unwrap() };

    m1.unlock().unwrap();

    // Not locked
    assert_eq!(
        m1.unlock(),
        Err(constance::kernel::UnlockMutexError::NotOwner)
    );

    // Double lock
    m1.lock().unwrap();
    assert_eq!(
        m1.lock(),
        Err(constance::kernel::LockMutexError::WouldDeadlock)
    );
    assert_eq!(
        m1.try_lock(),
        Err(constance::kernel::TryLockMutexError::WouldDeadlock)
    );
    assert_eq!(
        m1.lock_timeout(Duration::ZERO),
        Err(constance::kernel::LockMutexTimeoutError::WouldDeadlock)
    );
    m1.unlock().unwrap();

    // Correct locking/unlocking order
    {
        log::debug!("Doing the locking order stress test");
        let mut rng = Xorshift32(0xc0ffee00);
        let mut free = (1u32 << N) - 1; // 0b1111
        let mut locked: StaticVec<usize, N> = StaticVec::new();
        for i in (0..100).rev() {
            log::trace!("  locked = {:?}", locked);

            // All held mutexes but the last one should be prevented from being
            // unlocked
            for &[i, _] in locked.array_windows::<2>() {
                log::trace!("  making sure m[{}] is unlockable at this point", i);
                assert_eq!(
                    app.m[i].unlock(),
                    Err(constance::kernel::UnlockMutexError::BadObjectState)
                );
            }

            // Double lock
            for &i in locked.iter() {
                assert_eq!(
                    app.m[i].lock(),
                    Err(constance::kernel::LockMutexError::WouldDeadlock)
                );
                assert!(app.m[i].is_locked().unwrap());
            }

            let new_level = if i == 0 {
                // Unlock all mutexes on the last iteration
                0
            } else {
                rng.next() as usize % app.m.len()
            };
            log::trace!("  new_level = {}", new_level);

            while new_level < locked.len() {
                // Unlock the last held mutex
                let i = locked.pop().unwrap();
                log::trace!("  unlocking m[{:?}]", i);
                app.m[i].unlock().unwrap();
                free |= 1 << i;
            }

            while new_level > locked.len() {
                // Choose the next mutex to lock
                let mut i = free;
                for _ in 0..rng.next() % i.count_ones() {
                    i &= i - 1; // remove the lowest set bit
                }
                let i = i.trailing_zeros() as usize; // get the lowest set bit

                // Choose the method to use
                let method = (rng.next() & 0xff) % 3;

                log::trace!("  locking m[{:?}] using method {:?})", i, method);
                let m = app.m[i];
                match method {
                    0 => m.lock().unwrap(),
                    1 => m.try_lock().unwrap(),
                    2 => m.lock_timeout(Duration::from_millis(500)).unwrap(),
                    _ => unreachable!(),
                }
                free &= !(1u32 << i);
                locked.push(i);
            }
        }
    }

    // Already consistent
    assert_eq!(
        m1.mark_consistent(),
        Err(constance::kernel::MarkConsistentMutexError::BadObjectState)
    );

    // TODO: test prioity ceiling errors

    // Abandon `m1`
    m1.lock().unwrap();
    app.seq.expect_and_replace(2, 3);

    // Let task2 run
}

fn task2_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let app = D::app();
    let [m1, ..] = app.m;

    app.seq.expect_and_replace(3, 4);

    // `m1` was abandoned by `task1`.
    assert!(!m1.is_locked().unwrap());
    assert_eq!(m1.lock(), Err(constance::kernel::LockMutexError::Abandoned));

    // When `Abandoned` is returned, the ownership is given to the calling task
    // (This doesn't happen for other kinds of errors)
    assert!(m1.is_locked().unwrap());

    m1.unlock().unwrap();

    // The "abandoned" status lasts until it's explicitly cleared
    assert_eq!(m1.lock(), Err(constance::kernel::LockMutexError::Abandoned));
    m1.unlock().unwrap();
    assert_eq!(
        m1.try_lock(),
        Err(constance::kernel::TryLockMutexError::Abandoned)
    );
    m1.unlock().unwrap();
    assert_eq!(
        m1.lock_timeout(Duration::ZERO),
        Err(constance::kernel::LockMutexTimeoutError::Abandoned)
    );
    m1.unlock().unwrap();

    // Clear the "abandoned" status. `lock` will now return without an error
    m1.mark_consistent().unwrap();
    m1.lock().unwrap();

    D::success();
}

fn isr<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let app = D::app();
    let [m1, ..] = app.m;

    app.seq.expect_and_replace(1, 2);

    // Allowed in a non-task context
    assert_eq!(m1.is_locked(), Ok(false));
    assert_eq!(
        m1.mark_consistent(),
        Err(constance::kernel::MarkConsistentMutexError::BadObjectState)
    );

    // Disallowed in a non-task context
    assert_eq!(
        m1.unlock(),
        Err(constance::kernel::UnlockMutexError::BadContext)
    );
    assert_eq!(
        m1.lock(),
        Err(constance::kernel::LockMutexError::BadContext)
    );
    assert_eq!(
        m1.try_lock(),
        Err(constance::kernel::TryLockMutexError::BadContext)
    );
    assert_eq!(
        m1.lock_timeout(Duration::ZERO),
        Err(constance::kernel::LockMutexTimeoutError::BadContext)
    );
}

struct Xorshift32(u32);

impl Xorshift32 {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}
