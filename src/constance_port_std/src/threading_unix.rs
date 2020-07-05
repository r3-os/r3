use std::{
    ptr::null_mut,
    sync::{
        atomic::{AtomicPtr, Ordering},
        Arc,
    },
    thread,
};

pub use self::thread::ThreadId;

pub unsafe fn exit_thread() -> ! {
    unsafe {
        libc::pthread_exit(std::ptr::null_mut());
    }
}

/// `JoinHandle` with added functionalities.
#[derive(Debug)]
pub struct JoinHandle<T> {
    std_handle: thread::JoinHandle<T>,
    thread: Thread,
}

/// Spawn a new thread.
pub fn spawn<T: 'static + Send>(f: impl FnOnce() -> T + Send + 'static) -> JoinHandle<T> {
    let data = Arc::new(ThreadData {});
    let data2 = Arc::clone(&data);

    let std_handle = thread::spawn(move || {
        // Set up a destructor for `THREAD_DATA`
        THREAD_DATA_DTOR.with(|_| {});

        // Move `data2` into `THREAD_DATA`
        THREAD_DATA.store(Arc::into_raw(data2) as _, Ordering::Relaxed);

        f()
    });

    let thread = Thread {
        std_thread: std_handle.thread().clone(),
        data,
    };

    JoinHandle { std_handle, thread }
}

impl<T> JoinHandle<T> {
    pub fn thread(&self) -> &Thread {
        &self.thread
    }
}

// Avoid `pthread_getspecific`, which is not defined as async-signal-safe by
// the POSIX standard.
#[thread_local]
static THREAD_DATA: AtomicPtr<ThreadData> = AtomicPtr::new(null_mut());

// Releases `ThreadData` on thread exit.
thread_local! {
    static THREAD_DATA_DTOR: ThreadDataDestructor = ThreadDataDestructor;
}

struct ThreadDataDestructor;

impl Drop for ThreadDataDestructor {
    fn drop(&mut self) {
        // Take `Arc<_>` back from `THREAD_DATA`.
        let ptr = THREAD_DATA.swap(null_mut(), Ordering::Relaxed);
        if !ptr.is_null() {
            unsafe { Arc::from_raw(ptr) };
        }
    }
}

#[derive(Debug, Clone)]
pub struct Thread {
    std_thread: thread::Thread,
    data: Arc<ThreadData>,
}

#[derive(Debug)]
struct ThreadData {}

pub fn current() -> Thread {
    let data_ptr = THREAD_DATA.load(Ordering::Relaxed);

    let data = if data_ptr.is_null() {
        // The current thread was created in some other way. Construct
        // `ThreadData` now.
        let data = Arc::new(ThreadData {});
        let data2 = Arc::clone(&data);
        THREAD_DATA.store(Arc::into_raw(data2) as _, Ordering::Relaxed);

        // Set up a destructor for `THREAD_DATA`
        THREAD_DATA_DTOR.with(|_| {});

        data
    } else {
        let data = std::mem::ManuallyDrop::new(unsafe { Arc::from_raw(data_ptr) });
        Arc::clone(&data)
    };

    Thread {
        std_thread: thread::current(),
        data,
    }
}

pub fn park() {
    thread::park();
}

impl Thread {
    pub fn id(&self) -> ThreadId {
        self.std_thread.id()
    }

    pub fn unpark(&self) {
        self.std_thread.unpark();
    }

    // TODO: `park` (remote park)
}
