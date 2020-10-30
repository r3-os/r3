//! Validates error codes returned by mutex manipulation methods. Also,
//! checks miscellaneous properties of [`constance::kernel::Mutex`].
use constance::{
    hunk::Hunk,
    kernel::{cfg::CfgBuilder, InterruptHandler, InterruptLine, Mutex, MutexProtocol, Task},
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
    task2: Task<System>,
    task3: Task<System>,
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

        let task2 = Task::build()
            .start(task2_body::<System, D>)
            .priority(3)
            .finish(b);

        let task3 = Task::build()
            .start(task3_body::<System, D>)
            .priority(2)
            .finish(b);

        let m = [
            Mutex::build().protocol(MutexProtocol::None).finish(b),
            Mutex::build().protocol(MutexProtocol::Ceiling(1)).finish(b),
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

        App {
            task2,
            task3,
            int,
            m,
            seq,
        }
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

    #[cfg(feature = "priority_boost")]
    {
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
    }

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

    // Priority ceiling precondition
    // ----------------------------------------------------------------

    // `m2` uses the priority ceiling `1`. Let's use this to test the errors
    // specific to the priority ceiling protocol.
    let cur_task: Task<System> = Task::current().unwrap().unwrap();
    for pri in 0..=3 {
        let exceeds_ceiling = pri < 1;
        log::trace!(
            "set_priority({}) exceeds_ceiling = {:?}",
            pri,
            exceeds_ceiling
        );
        cur_task.set_priority(pri).unwrap();

        assert_eq!(cur_task.priority().unwrap(), pri);
        assert_eq!(cur_task.effective_priority().unwrap(), pri);

        if exceeds_ceiling {
            // The current priority exceeds the priority ceiling. Locking
            // operations will fail.
            assert_eq!(m2.lock(), Err(constance::kernel::LockMutexError::BadParam));
            assert_eq!(
                m2.try_lock(),
                Err(constance::kernel::TryLockMutexError::BadParam)
            );
            assert_eq!(
                m2.lock_timeout(Duration::ZERO),
                Err(constance::kernel::LockMutexTimeoutError::BadParam)
            );
        } else {
            // The current priority does not exceed the priority ceiling.
            // Locking operations should succeed.
            m2.lock().unwrap();
            assert_eq!(cur_task.priority().unwrap(), pri);
            assert_eq!(cur_task.effective_priority().unwrap(), 1);
            m2.unlock().unwrap();

            m2.try_lock().unwrap();
            m2.unlock().unwrap();

            m2.lock_timeout(Duration::ZERO).unwrap();

            // When holding a mutex lock, raising the task priority is also
            // restricted according to the locking protocol's precondition.
            for pri2 in 0..=3 {
                let exceeds_ceiling = pri2 < 1;
                log::trace!(
                    "  set_priority({}) exceeds_ceiling = {:?}",
                    pri2,
                    exceeds_ceiling
                );
                if exceeds_ceiling {
                    assert_eq!(
                        cur_task.set_priority(pri2),
                        Err(constance::kernel::SetTaskPriorityError::BadParam)
                    );
                } else {
                    cur_task.set_priority(pri2).unwrap();
                    assert_eq!(cur_task.priority().unwrap(), pri2);
                    assert_eq!(cur_task.effective_priority().unwrap(), 1);
                }
            }

            m2.unlock().unwrap();

            assert_eq!(cur_task.priority().unwrap(), 3);
            assert_eq!(cur_task.effective_priority().unwrap(), 3);
        }
    }

    cur_task.set_priority(2).unwrap();

    // Let `task3` block waiting upon `m2`.
    app.task3.activate().unwrap();
    m2.lock().unwrap();
    System::sleep(Duration::from_millis(200)).unwrap();

    assert_eq!(app.task3.priority().unwrap(), 2);
    assert_eq!(app.task3.effective_priority().unwrap(), 2);

    // When a task is waiting upon a mutex, raising its priority is also
    // restricted according to the locking protocol's precondition.
    for pri in (0..=3).rev() {
        let exceeds_ceiling = pri < 1;
        log::trace!(
            "task3.set_priority({}) exceeds_ceiling = {:?}",
            pri,
            exceeds_ceiling
        );
        if exceeds_ceiling {
            assert_eq!(
                app.task3.set_priority(pri),
                Err(constance::kernel::SetTaskPriorityError::BadParam)
            );
        } else {
            app.task3.set_priority(pri).unwrap();
            assert_eq!(app.task3.priority().unwrap(), pri);
            assert_eq!(app.task3.effective_priority().unwrap(), pri);
        }
    }

    // Let `task3` have the mutex lock. (`task3` is running at priority 1, so it
    // will immediately preempt the current task.)
    m2.unlock().unwrap();

    // `task3` will abandon `m2`. Clear the abandonment flag.
    m2.mark_consistent().unwrap();

    // Activate `task3` again. This will check that the effective priority
    // is reset to the initial value on task activation.
    app.task3.activate().unwrap();
    System::sleep(Duration::from_millis(200)).unwrap();
    m2.mark_consistent().unwrap();

    // Abandonment
    // ----------------------------------------------------------------

    // Abandon `m1` and `m2`
    m1.lock().unwrap();
    app.seq.expect_and_replace(2, 3);

    // Run `task2`. It has a low priority, so it will execute after the current
    // task exits.
    app.task2.activate().unwrap();
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

fn task3_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let app = D::app();
    let [_, m2, ..] = app.m;

    let cur_task: Task<System> = Task::current().unwrap().unwrap();
    assert_eq!(cur_task.priority().unwrap(), 2);
    assert_eq!(cur_task.effective_priority().unwrap(), 2);

    m2.lock().unwrap();
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
