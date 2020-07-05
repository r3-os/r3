//! Test cases for `crate::threading`
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread::sleep,
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
