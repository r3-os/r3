//! Threading library similar to `std::thread` but supporting the remote park
//! operation ([`Thread::park`]).
use parking_lot::Mutex;
use std::{
    mem::MaybeUninit,
    os::raw::c_int,
    ptr::null_mut,
    sync::{
        atomic::{AtomicPtr, AtomicUsize, Ordering},
        Arc, Once,
    },
    thread,
};

pub use self::thread::ThreadId;

pub unsafe fn exit_thread() -> ! {
    unsafe {
        libc::pthread_exit(std::ptr::null_mut());
    }
}

/// [`std::thread::JoinHandle`] with extra functionalities.
#[derive(Debug)]
pub struct JoinHandle<T> {
    std_handle: thread::JoinHandle<T>,
    thread: Thread,
}

/// Spawn a new thread.
pub fn spawn<T: 'static + Send>(f: impl FnOnce() -> T + Send + 'static) -> JoinHandle<T> {
    let parent_thread = thread::current();

    let data = Arc::new(ThreadData::new());
    let data2 = Arc::clone(&data);

    let std_handle = thread::spawn(move || {
        // Set up a destructor for `THREAD_DATA`
        THREAD_DATA_DTOR.with(|_| {});

        data2.set_self();

        // Move `data2` into `THREAD_DATA`
        THREAD_DATA.store(Arc::into_raw(data2) as _, Ordering::Relaxed);

        parent_thread.unpark();
        drop(parent_thread);

        f()
    });

    let thread = Thread {
        std_thread: std_handle.thread().clone(),
        data,
    };

    // Wait until the just-spawned thread configures its own `THREAD_DATA`.
    thread::park();

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

/// [`std::thread::Thread`] with extra functionalities.
#[derive(Debug, Clone)]
pub struct Thread {
    std_thread: thread::Thread,
    data: Arc<ThreadData>,
}

#[derive(Debug)]
struct ThreadData {
    park_sock: [c_int; 2],
    pthread_id: AtomicUsize,
}

impl ThreadData {
    fn new() -> Self {
        let park_sock = unsafe {
            let mut park_sock = MaybeUninit::uninit();
            ok_or_errno(libc::socketpair(
                libc::PF_LOCAL,
                libc::SOCK_STREAM,
                0,
                park_sock.as_mut_ptr() as _,
            ))
            .unwrap();
            park_sock.assume_init()
        };

        let this = Self {
            park_sock,
            pthread_id: AtomicUsize::new(0),
        };

        // Enable non-blocking I/O
        // TODO: Non-blocking I/O isn't necessary anymore - just let `recv` block
        ok_or_errno(unsafe {
            libc::fcntl(
                this.park_sock_token_source(),
                libc::F_SETFL,
                libc::O_NONBLOCK,
            )
        })
        .unwrap();

        this
    }

    /// Assign `self.pthread_id` using `pthread_self`.
    fn set_self(&self) {
        self.pthread_id
            .store(unsafe { libc::pthread_self() }, Ordering::Relaxed);
    }

    /// Get the FD to read a park token.
    fn park_sock_token_source(&self) -> c_int {
        self.park_sock[0]
    }

    /// Get the FD to write a park token.
    fn park_sock_token_sink(&self) -> c_int {
        self.park_sock[1]
    }
}

impl Drop for ThreadData {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.park_sock[0]);
            libc::close(self.park_sock[1]);
        }
    }
}

pub fn current() -> Thread {
    let data_ptr = THREAD_DATA.load(Ordering::Relaxed);

    let data = if data_ptr.is_null() {
        // The current thread was created in some other way. Construct
        // `ThreadData` now.
        let data = Arc::new(ThreadData::new());
        let data2 = Arc::clone(&data);
        THREAD_DATA.store(Arc::into_raw(data2) as _, Ordering::Relaxed);

        // Set up a destructor for `THREAD_DATA`
        THREAD_DATA_DTOR.with(|_| {});

        data.set_self();

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
    let current = current();
    park_inner(&current.data);
}

fn park_inner(data: &ThreadData) {
    let mut buf = 0u8;

    loop {
        // Block the current thread until the token becomes available
        let mut pollfd = libc::pollfd {
            fd: data.park_sock_token_source(),
            events: libc::POLLRDNORM,
            revents: 0,
        };
        let count = ok_or_errno(unsafe { libc::poll(&mut pollfd, 1, c_int::MAX) }).unwrap();

        if count == 0 {
            // It's not available yet. Start waiting again
            continue;
        }

        // Take the token
        match isize_ok_or_errno(unsafe {
            libc::recv(
                data.park_sock_token_source(),
                (&mut buf) as *mut _ as _,
                1,
                0,
            )
        }) {
            Ok(1) => {}
            Ok(0) | Err(errno::Errno(libc::EAGAIN)) => {
                // It was a spurious wakeup (this can be caused by how `unpark`
                // is implemented). Try again.
                continue;
            }
            Ok(i) => panic!("unexpected return value: {}", i),
            Err(e) => panic!("failed to evict park token: {}", e),
        }

        break;
    }
}

impl Thread {
    pub fn id(&self) -> ThreadId {
        self.std_thread.id()
    }

    /// Make a new park token available for the thread.
    ///
    /// Unlike [`std::thread::Thread::unpark`], **a thread can have multiple
    /// tokens**. Each call to `park` will consume one token. The maximum number
    /// of tokens a thread can have is unspecified.
    pub fn unpark(&self) {
        let data = &self.data;

        // Make a token available
        isize_ok_or_errno(unsafe {
            libc::send(data.park_sock_token_sink(), &0u8 as *const _ as _, 1, 0)
        })
        .unwrap();
    }

    /// Force the thread to park.
    ///
    /// The effect is equivalent to calling `park` on the target thread.
    /// However, this method can be called from any thread. (I call this “remote
    /// park”.)
    ///
    /// The result is unspecified if the thread has already exited.
    pub fn park(&self) {
        // Make sure the signal handler is registered
        static SIGNAL_HANDLER_ONCE: Once = Once::new();
        SIGNAL_HANDLER_ONCE.call_once(register_remote_park_signal_handler);

        let pthread_id = self.data.pthread_id.load(Ordering::Relaxed);

        // Raise the signal `SIGNAL_REMOTE_PARK`. This will force the target
        // thread to execute `remote_park_signal_handler`.
        ok_or_errno(unsafe { libc::pthread_kill(pthread_id, SIGNAL_REMOTE_PARK) }).unwrap();
    }
}

const SIGNAL_REMOTE_PARK: c_int = libc::SIGUSR1;

/// Register the signal handler for `SIGNAL_REMOTE_PARK`.
#[cold]
fn register_remote_park_signal_handler() {
    ok_or_errno(unsafe {
        libc::sigaction(
            SIGNAL_REMOTE_PARK,
            &libc::sigaction {
                sa_sigaction: remote_park_signal_handler as libc::sighandler_t,
                sa_mask: 0,
                sa_flags: libc::SA_SIGINFO,
            },
            null_mut(),
        )
    })
    .unwrap();

    /// The signal handler for `SIGNAL_REMOTE_PARK`.
    extern "C" fn remote_park_signal_handler(
        _signo: c_int,
        _: *mut libc::siginfo_t,
        _: *mut libc::ucontext_t,
    ) {
        let current_ptr = THREAD_DATA.load(Ordering::Relaxed);
        assert!(!current_ptr.is_null());
        let current = unsafe { &*current_ptr };

        // Park the current thread
        park_inner(current);
    }
}

fn isize_ok_or_errno(x: isize) -> Result<isize, errno::Errno> {
    if x >= 0 {
        Ok(x)
    } else {
        Err(errno::errno())
    }
}

fn ok_or_errno(x: c_int) -> Result<c_int, errno::Errno> {
    if x >= 0 {
        Ok(x)
    } else {
        Err(errno::errno())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread::sleep, time::Duration};

    /// Make sure that the child thread dereferences `ThreadData` when it exits
    /// by returning.
    #[test]
    fn returning_releases_thread_data() {
        let jh = spawn(|| {
            assert_eq!(Arc::strong_count(&current().data), 3);
            assert_eq!(Arc::strong_count(&current().data), 3);
        });

        // Wait until the child thread exits
        sleep(Duration::from_millis(200));

        // `jh` should be the sole owner of `ThreadData` now
        assert_eq!(Arc::strong_count(&jh.thread.data), 1);
    }

    /// Make sure that the child thread dereferences `ThreadData` when it exits
    /// by `exit_thread`.
    ///
    /// This property is important because that's the sole way for our task
    /// thread to exit, and `ThreadData` includes file descriptors, which are
    /// (relatively) scarce resources.
    #[test]
    fn exit_thread_releases_thread_data() {
        let jh = spawn(|| {
            assert_eq!(Arc::strong_count(&current().data), 3);
            assert_eq!(Arc::strong_count(&current().data), 3);
            unsafe { exit_thread() };
        });

        // Wait until the child thread exits
        sleep(Duration::from_millis(200));

        // `jh` should be the sole owner of `ThreadData` now
        assert_eq!(Arc::strong_count(&jh.thread.data), 1);
    }
}
