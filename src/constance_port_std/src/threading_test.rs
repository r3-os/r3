//! Test cases for `crate::threading`
use std::{
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
    thread::{sleep, yield_now},
    time::Duration,
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
fn remote_park() {
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

    // Suspend the child thread
    jh.thread().park();

    // Check that the child thread is not running
    let i1 = counter.load(Ordering::Relaxed);
    yield_now();
    sleep(Duration::from_millis(200));
    yield_now();
    let i2 = counter.load(Ordering::Relaxed);
    assert_eq!(i1, i2);

    // Resume the child thread
    jh.thread().unpark();

    // Check that the child thread is running
    let i1 = counter.load(Ordering::Relaxed);
    yield_now();
    sleep(Duration::from_millis(200));
    yield_now();
    let i2 = counter.load(Ordering::Relaxed);
    assert_ne!(i1, i2);

    // This should be no-op
    jh.thread().unpark(); // Make a token available
    jh.thread().park(); // Immediately consume that token

    // Stop the child thread (this should work assuming that the child thread
    // is still running)
    exit.store(true, Ordering::Relaxed);

    // Wait for the child thread to exit
    threading::park();
    assert!(done.load(Ordering::Relaxed));
}
