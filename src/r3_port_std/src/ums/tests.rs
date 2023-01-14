use super::*;
use std::{
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    sync::OnceLock,
    thread::sleep,
    time::{Duration, Instant},
};

fn init_logger() {
    // `is_test(true)` would drop log messages from other threads
    let _ = env_logger::try_init();
}

/// Validates the UMS behavior by doing some sort of preemptive round-robin
/// scheduling.
#[test]
fn preempt() {
    init_logger();

    struct St {
        counters: [AtomicUsize; 3],
        done: AtomicBool,
        cur_thread: AtomicUsize,
        threads: OnceLock<[ThreadId; 3]>,
    }
    let st: &_ = Box::leak(Box::new(St {
        counters: [
            AtomicUsize::new(0),
            AtomicUsize::new(0),
            AtomicUsize::new(0),
        ],
        done: AtomicBool::new(false),
        cur_thread: AtomicUsize::new(0),
        threads: OnceLock::new(),
    }));

    impl Scheduler for &'static St {
        fn choose_next_thread(&mut self) -> Option<ThreadId> {
            let threads = self.threads.get().unwrap();
            threads
                .get(self.cur_thread.load(Ordering::Relaxed))
                .cloned()
        }
    }

    let (tg, join_handle) = ThreadGroup::new(st);

    {
        let mut lock = tg.lock();

        let t0 = lock.spawn(move |_| {
            while !st.done.load(Ordering::Relaxed) {
                st.counters[0].fetch_add(1, Ordering::Relaxed);
            }

            // Schedule no thread
            st.cur_thread.store(usize::MAX, Ordering::Relaxed);
        });

        let t1 = lock.spawn(move |_| {
            while !st.done.load(Ordering::Relaxed) {
                st.counters[1].fetch_add(1, Ordering::Relaxed);
            }

            // Schedule thread 0
            st.cur_thread.store(0, Ordering::Relaxed);
        });

        let t2 = lock.spawn(move |_| {
            while !st.done.load(Ordering::Relaxed) {
                st.counters[2].fetch_add(1, Ordering::Relaxed);
            }

            // Schedule thread 1
            st.cur_thread.store(1, Ordering::Relaxed);
        });

        st.threads.set([t0, t1, t2]).unwrap();
    }

    let schedule_thread = |thread_i| {
        // Schedule thread `thread_i`
        st.cur_thread.store(thread_i, Ordering::Relaxed);

        tg.lock().preempt();

        // Only thread `thread_i` should be running
        let old_counters: Vec<_> = st
            .counters
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .collect();

        sleep(Duration::from_millis(100));

        let new_counters: Vec<_> = st
            .counters
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .collect();

        assert!(
            old_counters
                .iter()
                .zip(new_counters.iter())
                .enumerate()
                .all(|(i, (old, new))| (thread_i == i) == (old != new)),
            "old_counters = {old_counters:?}, new_counters = {new_counters:?}",
        );
    };

    schedule_thread(0);
    schedule_thread(1);
    schedule_thread(2);

    st.done.store(true, Ordering::Relaxed);
    tg.lock().shutdown();

    join_handle.join().unwrap();
}

#[test]
fn yield_ring_2() {
    yield_ring(2);
}

#[test]
fn yield_ring_5() {
    yield_ring(5);
}

#[test]
fn yield_ring_100() {
    yield_ring(100);
}

/// Validates the UMS behavior by simulating token passing.
fn yield_ring(count: usize) {
    init_logger();

    struct St {
        counters: Vec<AtomicUsize>,
        done: AtomicBool,
        cur_thread: AtomicUsize,
        threads: OnceLock<Vec<ThreadId>>,
    }
    let st: &_ = Box::leak(Box::new(St {
        counters: (0..count).map(|_| AtomicUsize::new(0)).collect(),
        done: AtomicBool::new(false),
        cur_thread: AtomicUsize::new(0),
        threads: OnceLock::new(),
    }));

    const COUNTER_THREAD_ENDED: usize = usize::MAX;

    impl Scheduler for &'static St {
        fn choose_next_thread(&mut self) -> Option<ThreadId> {
            let threads = self.threads.get().unwrap();
            if let Some(&thread_id) = threads.get(self.cur_thread.load(Ordering::Relaxed)) {
                Some(thread_id)
            } else {
                // Schedule any alive thread
                self.counters
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i, c.load(Ordering::Relaxed)))
                    .find(|(_, c)| *c != COUNTER_THREAD_ENDED)
                    .map(|(i, _)| threads[i])
            }
        }
    }

    let (tg, join_handle) = ThreadGroup::new(st);

    {
        let mut lock = tg.lock();

        let threads = (0..count)
            .map(|i| {
                lock.spawn(move |_| {
                    while !st.done.load(Ordering::Relaxed) {
                        assert_eq!(st.cur_thread.load(Ordering::Relaxed), i);

                        st.counters[i].fetch_add(1, Ordering::Relaxed);

                        // Schedule the next thread
                        st.cur_thread.store((i + 1) % count, Ordering::Relaxed);
                        yield_now();
                    }

                    // Schedule any alive thread
                    st.cur_thread.store(usize::MAX, Ordering::Relaxed);

                    // Mark this thread as dead
                    st.counters[i].store(COUNTER_THREAD_ENDED, Ordering::Relaxed);
                })
            })
            .collect();

        st.threads.set(threads).unwrap();
    }

    // Start the first thread
    tg.lock().preempt();

    // Wait for a while...
    let duration: u64 = 400_000_000;
    sleep(Duration::from_nanos(duration));

    // All threads must have run
    let new_counters: Vec<_> = st
        .counters
        .iter()
        .map(|c| c.load(Ordering::Relaxed))
        .collect();
    let sum: usize = new_counters.iter().sum();

    log::info!(
        "new_counters = {new_counters:?}, sum = {sum:?} ({:?} ns/iter)",
        duration / sum as u64
    );

    assert!(
        new_counters.iter().all(|&c| c != 0),
        "new_counters = {new_counters:?}",
    );

    st.done.store(true, Ordering::Relaxed);
    tg.lock().shutdown();

    join_handle.join().unwrap();
}

/// Calls `preempt` (with no effect) in a rapid succession and makes sure
/// nothing breaks.
#[test]
fn preempt_rapid() {
    init_logger();

    struct St {
        done: AtomicBool,
        threads: OnceLock<ThreadId>,
    }
    let st: &_ = Box::leak(Box::new(St {
        done: AtomicBool::new(false),
        threads: OnceLock::new(),
    }));

    impl Scheduler for &'static St {
        fn choose_next_thread(&mut self) -> Option<ThreadId> {
            // `choose_next_thread` shouldn't be called after shutdown
            // completion, so we don't have to check that the thread still
            // exists.
            Some(self.threads.get().cloned().unwrap())
        }
    }

    let (tg, join_handle) = ThreadGroup::new(st);

    {
        let mut lock = tg.lock();
        let t0 = lock.spawn(move |_| {
            while !st.done.load(Ordering::Relaxed) {
                std::hint::spin_loop();
            }
        });
        st.threads.set(t0).unwrap();
    }

    // Schedule the worker thread
    tg.lock().preempt();

    // Shut down the thread group as soon as the worker thread exits.
    tg.lock().shutdown();

    let start = Instant::now();
    while start.elapsed().as_millis() < 100 {
        // Request rescheduling, which will have no effect
        tg.lock().preempt();
    }

    st.done.store(true, Ordering::Relaxed);

    join_handle.join().unwrap();
}

/// See that a panic is propagated to the main thread
#[test]
#[should_panic]
fn forward_panic() {
    init_logger();

    struct St {
        threads: OnceLock<ThreadId>,
    }
    let st: &_ = Box::leak(Box::new(St {
        threads: OnceLock::new(),
    }));

    impl Scheduler for &'static St {
        fn choose_next_thread(&mut self) -> Option<ThreadId> {
            Some(self.threads.get().cloned().unwrap())
        }
    }

    let (tg, join_handle) = ThreadGroup::new(st);

    {
        let mut lock = tg.lock();
        let t0 = lock.spawn(move |_| panic!("blah"));
        st.threads.set(t0).unwrap();
    }

    // Schedule the thread
    tg.lock().preempt();

    // This should panic
    join_handle.join().unwrap();
}

/// Tests two ways of exiting a thread.
#[test]
fn exit_current_thread() {
    init_logger();

    let count = 10;

    struct Sched {
        threads: Vec<ThreadId>,
    }

    impl Scheduler for Sched {
        fn choose_next_thread(&mut self) -> Option<ThreadId> {
            // Schedule any alive thread
            self.threads.first().cloned()
        }

        fn thread_exited(&mut self, thread_id: ThreadId) {
            let i = self.threads.iter().position(|t| *t == thread_id).unwrap();
            self.threads.remove(i);
        }
    }

    let (tg, join_handle) = ThreadGroup::new(Sched {
        threads: Vec::new(),
    });

    {
        let mut lock = tg.lock();

        let threads = (0..count)
            .map(|i| {
                lock.spawn(move |_| {
                    if i % 2 == 0 {
                        // Exit the thread by calling `exit_thread`
                        unsafe { exit_thread() };
                    } else {
                        // Exit the thread by returning
                    }
                })
            })
            .collect();

        lock.scheduler().threads = threads;
    }

    // Shut down the thread group as soon as the worker thread exits.
    tg.lock().shutdown();

    // Start the scheduling. All threads will evetually exit, but for this to
    // happen, the scheduler should schedule each thread at least once.
    tg.lock().preempt();

    join_handle.join().unwrap();
}
