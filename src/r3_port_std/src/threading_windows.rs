use std::{
    mem::MaybeUninit,
    ptr::addr_of,
    sync::{
        atomic::{AtomicIsize, Ordering},
        mpsc, Arc,
    },
    thread,
};
use winapi::um::{
    errhandlingapi, handleapi, processthreadsapi, synchapi,
    winbase::INFINITE,
    winnt::{DUPLICATE_SAME_ACCESS, HANDLE},
};

pub unsafe fn exit_thread() -> ! {
    unsafe { processthreadsapi::ExitThread(0) };
    unreachable!();
}

pub use std::thread::ThreadId;

/// [`std::thread::JoinHandle`] with extra functionalities.
#[derive(Debug)]
pub struct JoinHandle<T> {
    _std_handle: thread::JoinHandle<T>,
    thread: Thread,
}

/// Spawn a new thread.
pub fn spawn(f: impl FnOnce() + Send + 'static) -> JoinHandle<()> {
    let (send, recv) = mpsc::channel();

    let std_handle = thread::spawn(move || {
        // Send `Arc<ThreadData>`
        let _ = send.send(THREAD_DATA.with(Arc::clone));

        f()
    });

    let data = recv.recv().unwrap();

    let thread = Thread { data };

    JoinHandle {
        _std_handle: std_handle,
        thread,
    }
}

impl<T> JoinHandle<T> {
    pub fn thread(&self) -> &Thread {
        &self.thread
    }
}
thread_local! {
    static THREAD_DATA: Arc<ThreadData> = Arc::new(ThreadData {
        token_count: AtomicIsize::new(0),
        hthread: current_hthread(),
        remote_op_mutex: Mutex::new(()),
    });
}

/// [`std::thread::Thread`] with extra functionalities.
#[derive(Debug, Clone)]
pub struct Thread {
    data: Arc<ThreadData>,
}

#[derive(Debug)]
struct ThreadData {
    token_count: AtomicIsize,
    hthread: HANDLE,
    remote_op_mutex: Mutex<()>,
}

unsafe impl Send for ThreadData {}
unsafe impl Sync for ThreadData {}

#[allow(dead_code)]
pub fn current() -> Thread {
    Thread {
        data: THREAD_DATA.with(Arc::clone),
    }
}

pub fn park() {
    THREAD_DATA.with(|td| {
        let token_count_cell = &td.token_count;
        let mut token_count = token_count_cell.fetch_sub(1, Ordering::Relaxed) - 1;
        while token_count < 0 {
            unsafe {
                synchapi::WaitOnAddress(
                    token_count_cell.as_mut_ptr().cast(),    // location to watch
                    addr_of!(token_count).cast_mut().cast(), // undesired value
                    std::mem::size_of::<isize>(),            // value size
                    INFINITE,                                // timeout
                );
            }
            token_count = token_count_cell.load(Ordering::Relaxed);
        }
    })
}

impl Thread {
    /// Make a new park token available for the thread.
    ///
    /// Unlike [`std::thread::Thread::unpark`], **a thread can have multiple
    /// tokens**. Each call to `park` will consume one token. The maximum number
    /// of tokens a thread can have is unspecified.
    ///
    /// This implementation is not lock-free. If your remote-park a thread
    /// executing this method, other remote-park or unpark operations for the
    /// same thread may be prevented from making progress.
    pub fn unpark(&self) {
        let _guard = self.data.remote_op_mutex.lock().unwrap();
        let token_count_cell = &self.data.token_count;
        if token_count_cell.fetch_add(1, Ordering::Relaxed) == -1 {
            unsafe { synchapi::WakeByAddressAll(token_count_cell.as_mut_ptr().cast()) };
            unsafe { processthreadsapi::ResumeThread(self.data.hthread) };
        }
    }

    /// Force the thread to park.
    ///
    /// The effect is equivalent to calling `park` on the target thread.
    /// However, this method can be called from any thread. (I call this “remote
    /// park”.)
    ///
    /// The result is unspecified if the thread has already exited.
    ///
    /// This implementation is not lock-free. If your remote-park a thread
    /// executing this method, other remote-park or unpark operations for the
    /// same thread may be prevented from making progress. This also implies
    /// that this method shouldn't be used to park the current thread, and the
    /// [`park`] global function should be used instead.
    pub fn park(&self) {
        let _guard = self.data.remote_op_mutex.lock().unwrap();
        let token_count_cell = &self.data.token_count;
        if token_count_cell.fetch_sub(1, Ordering::Relaxed) == 0 {
            unsafe { processthreadsapi::SuspendThread(self.data.hthread) };

            // Wait for the suspend request to complete
            // <https://devblogs.microsoft.com/oldnewthing/20150205-00/?p=44743>
            unsafe {
                processthreadsapi::GetThreadContext(
                    self.data.hthread,
                    MaybeUninit::uninit().as_mut_ptr(),
                );
            }
        }
    }
}

fn current_hthread() -> HANDLE {
    // pseudo handle, which is converted to a "real" handle by
    // `DuplicateHandle`.
    let cur_pseudo_hthread = unsafe { processthreadsapi::GetCurrentThread() };

    let cur_hprocess = unsafe { processthreadsapi::GetCurrentProcess() };
    let mut cur_hthread = MaybeUninit::uninit();
    assert_win32_ok(unsafe {
        handleapi::DuplicateHandle(
            cur_hprocess,
            cur_pseudo_hthread, // source handle
            cur_hprocess,
            cur_hthread.as_mut_ptr(), // target handle
            0,                        // desired access - ignored because of `DUPLICATE_SAME_ACCESS`
            0,                        // do not inherit
            DUPLICATE_SAME_ACCESS,
        )
    });

    assert_win32_nonnull(unsafe { cur_hthread.assume_init() })
}

fn assert_win32_ok<T: Default + PartialEq<T> + Copy>(b: T) {
    if b == T::default() {
        panic_last_error();
    }
}

/// Panic with an error code returned by `GetLastError` if the
/// given pointer is null.
fn assert_win32_nonnull<T: IsNull>(b: T) -> T {
    if b.is_null() {
        panic_last_error();
    }
    b
}

trait IsNull {
    fn is_null(&self) -> bool;
}

impl<T: ?Sized> IsNull for *const T {
    fn is_null(&self) -> bool {
        (*self).is_null()
    }
}
impl<T: ?Sized> IsNull for *mut T {
    fn is_null(&self) -> bool {
        (*self).is_null()
    }
}

/// Panic with an error code returned by `GetLastError`.
#[cold]
fn panic_last_error() -> ! {
    panic!("Win32 error 0x{:08x}", unsafe {
        errhandlingapi::GetLastError()
    });
}

// `std::sync::Mutex` isn't remote-park-friendly
pub use mutex::{Mutex, MutexGuard};

mod mutex {
    use std::{
        cell::UnsafeCell,
        fmt,
        sync::atomic::{AtomicBool, Ordering},
    };
    use winapi::um::{synchapi, winbase::INFINITE};

    /// Remote-park-friendly [`std::sync::Mutex`].
    pub struct Mutex<T: ?Sized> {
        locked: AtomicBool,
        data: UnsafeCell<T>,
    }

    unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

    pub struct MutexGuard<'a, T: ?Sized> {
        data: &'a mut T,
        locked: &'a AtomicBool,
    }

    impl<T> Mutex<T> {
        #[inline]
        pub const fn new(x: T) -> Self {
            Self {
                data: UnsafeCell::new(x),
                locked: AtomicBool::new(false),
            }
        }
    }

    impl<T: ?Sized> Mutex<T> {
        #[inline]
        pub fn lock(&self) -> Result<MutexGuard<'_, T>, ()> {
            while self
                .locked
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
            {
                unsafe {
                    synchapi::WaitOnAddress(
                        self.locked.as_mut_ptr().cast(),   // location to watch
                        [true].as_ptr().cast_mut().cast(), // undesired value
                        std::mem::size_of::<bool>(),       // value size
                        INFINITE,                          // timeout
                    );
                }
            }

            // Poisoning is not supported by this `Mutex`

            Ok(MutexGuard {
                data: unsafe { &mut *self.data.get() },
                locked: &self.locked,
            })
        }
    }

    impl<T: ?Sized + fmt::Debug> fmt::Debug for Mutex<T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("Mutex")
        }
    }

    impl<T: ?Sized> Drop for MutexGuard<'_, T> {
        #[inline]
        fn drop(&mut self) {
            self.locked.store(false, Ordering::Release);
            unsafe { synchapi::WakeByAddressSingle(self.locked.as_mut_ptr().cast()) };
        }
    }

    impl<T: ?Sized> std::ops::Deref for MutexGuard<'_, T> {
        type Target = T;

        #[inline]
        fn deref(&self) -> &Self::Target {
            self.data
        }
    }

    impl<T: ?Sized> std::ops::DerefMut for MutexGuard<'_, T> {
        #[inline]
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.data
        }
    }
}
