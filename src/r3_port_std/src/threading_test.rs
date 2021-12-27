//! Test cases for `crate::threading`
use quickcheck_macros::quickcheck;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
    thread::{sleep, yield_now},
    time::{Duration, Instant},
};

use super::threading;

#[test]
fn unpark_external_thread() {
    let parent_thread = threading::current();
    let f: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
    threading::spawn(move || {
        f.store(true, Ordering::Relaxed);
        // `parent_thread` wasn't created by `threading::spawn`, but this
        // should succeed
        parent_thread.unpark();
    });
    threading::park();
    assert!(f.load(Ordering::Relaxed));
}

#[test]
fn park_early() {
    let parent_thread = threading::current();
    let f: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));

    let jh = threading::spawn(move || {
        threading::park();
        assert!(f.load(Ordering::Relaxed));

        // Wake up the parent thread, signifying success
        parent_thread.unpark();
    });

    sleep(Duration::from_millis(100));
    f.store(true, Ordering::Relaxed);
    // Wake up the sleeping child thread
    jh.thread().unpark();

    threading::park();
}

#[test]
fn park_late() {
    let parent_thread = threading::current();
    let f: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));

    let jh = threading::spawn(move || {
        sleep(Duration::from_millis(100));
        threading::park();
        assert!(f.load(Ordering::Relaxed));

        // Wake up the parent thread, signifying success
        parent_thread.unpark();
    });

    f.store(true, Ordering::Relaxed);

    // Wake up the child thread, which probably hasn't yet parked
    jh.thread().unpark();

    threading::park();
}

#[test]
fn remote_park_properties() {
    let parent_thread = threading::current();
    let done: &_ = Box::leak(Box::new(AtomicBool::new(false)));
    let exit: &_ = Box::leak(Box::new(AtomicBool::new(false)));
    let counter: &_ = Box::leak(Box::new(AtomicU32::new(0)));

    let jh = threading::spawn(move || {
        while !exit.load(Ordering::Relaxed) {
            counter.fetch_add(1, Ordering::Relaxed);
        }

        done.store(true, Ordering::Relaxed);

        // Wake up the parent thread, signifying success
        parent_thread.unpark();
    });

    sleep(Duration::from_millis(200));

    // Suspend and resume the child thread in a rapid succession
    for _ in 0..1000 {
        jh.thread().park();
        jh.thread().unpark();
    }

    // Park a lot
    for _ in 0..1000 {
        jh.thread().park();
    }
    for _ in 0..1000 {
        jh.thread().unpark();
    }

    // Check that the child thread is running
    let i1 = counter.load(Ordering::Relaxed);
    yield_now();
    sleep(Duration::from_millis(200));
    yield_now();
    let i2 = counter.load(Ordering::Relaxed);
    assert_ne!(i1, i2);

    for _ in 0..1000 {
        // Suspend the child thread
        jh.thread().park();

        // Check that the child thread is not running
        let i1 = counter.load(Ordering::Relaxed);
        yield_now();
        let i2 = counter.load(Ordering::Relaxed);
        assert_eq!(i1, i2);

        // Resume the child thread
        jh.thread().unpark();

        // Check that the child thread is running
        let i1 = counter.load(Ordering::Relaxed);
        let start = Instant::now();
        let i2 = loop {
            yield_now();
            let i2 = counter.load(Ordering::Relaxed);
            if i1 != i2 || start.elapsed() > Duration::from_millis(20000) {
                break i2;
            }
        };
        assert_ne!(i1, i2);

        // This should be no-op
        jh.thread().unpark(); // Make a token available
        jh.thread().park(); // Immediately consume that token
    }

    // Stop the child thread (this should work assuming that the child thread
    // is still running)
    exit.store(true, Ordering::Relaxed);

    // Wait for the child thread to exit
    threading::park();
    assert!(done.load(Ordering::Relaxed));
}

#[quickcheck]
fn qc_remote_park_accumulation(ops: Vec<u8>) {
    let parent_thread = threading::current();
    let done = Arc::new(AtomicBool::new(false));
    let exit = Arc::new(AtomicBool::new(false));

    let done2 = Arc::clone(&done);
    let exit2 = Arc::clone(&exit);

    let jh = threading::spawn(move || {
        while !exit2.load(Ordering::Relaxed) {}

        done2.store(true, Ordering::Relaxed);

        // Wake up the parent thread, signifying success
        parent_thread.unpark();
    });

    let mut park_level = 0;
    for op in ops {
        if park_level < 0 || (op & 1 == 0) {
            park_level += 1;
            jh.thread().park();
        } else {
            park_level -= 1;
            jh.thread().unpark();
        }
    }

    for _ in 0..park_level {
        jh.thread().unpark();
    }

    // Stop the child thread (this should work assuming that the child thread
    // is still running)
    exit.store(true, Ordering::Relaxed);

    // Wait for the child thread to exit
    threading::park();
    assert!(done.load(Ordering::Relaxed));
}
