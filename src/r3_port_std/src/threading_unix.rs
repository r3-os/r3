//! Threading library similar to `std::thread` but supporting the remote park
//! operation ([`Thread::park`]).
use crate::utils::Atomic;
use std::{
    arch::asm,
    cell::Cell,
    mem::MaybeUninit,
    os::raw::c_int,
    ptr::{null_mut, NonNull},
    sync::{
        atomic::{AtomicPtr, AtomicUsize, Ordering},
        Arc, Once,
    },
    thread,
};

pub use self::thread::ThreadId;

thread_local! {
    static EXIT_JMP_BUF: Cell<Option<JmpBuf>> = Cell::new(None);
}

pub unsafe fn exit_thread() -> ! {
    let jmp_buf = EXIT_JMP_BUF
        .with(|c| c.get())
        .expect("this thread wasn't started by `threading::spawn`");
    unsafe { longjmp(jmp_buf) };
}

/// [`std::thread::JoinHandle`] with extra functionalities.
#[derive(Debug)]
pub struct JoinHandle<T> {
    std_handle: thread::JoinHandle<T>,
    thread: Thread,
}

/// Spawn a new thread.
pub fn spawn(f: impl FnOnce() + Send + 'static) -> JoinHandle<()> {
    let parent_thread = thread::current();

    let data = Arc::new(ThreadData::new());
    let data2 = Arc::clone(&data);

    let std_handle = thread::spawn(move || {
        // Set up a destructor for `THREAD_DATA`
        THREAD_DATA_DTOR.with(|_| {});

        data2.set_self();

        // Move `data2` into `THREAD_DATA`
        THREAD_DATA.store(Arc::into_raw(data2) as _, Ordering::Relaxed);

        catch_longjmp(move |jmp_buf| {
            EXIT_JMP_BUF.with(|c| c.set(Some(jmp_buf)));

            parent_thread.unpark();
            drop(parent_thread);

            f()
        });
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
    park_count: AtomicUsize,
    pthread_id: Atomic<libc::pthread_t>,
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

        Self {
            park_sock,
            park_count: AtomicUsize::new(0),
            pthread_id: Atomic::<libc::pthread_t>::new(0),
        }
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
    loop {
        // Take the token (blocking)
        match isize_ok_or_errno(unsafe {
            libc::recv(
                data.park_sock_token_source(),
                (&mut 0u8) as *mut _ as _,
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
            Err(errno::Errno(libc::EINTR)) => {
                // Interrupted while waiting. Try again.
                continue;
            }
            Ok(i) => panic!("unexpected return value: {}", i),
            Err(e) => panic!("failed to evict park token: {}", e),
        }

        break;
    }
}

impl Thread {
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

        self.data.park_count.fetch_add(1, Ordering::Relaxed);

        // Raise the signal `SIGNAL_REMOTE_PARK`. This will force the target
        // thread to execute `remote_park_signal_handler`.
        ok_or_errno(unsafe { libc::pthread_kill(pthread_id, SIGNAL_REMOTE_PARK) }).unwrap();

        // Wait until the signal is delivered.
        while self.data.park_count.load(Ordering::Relaxed) != 0 {
            std::thread::yield_now();
        }
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
                // `SA_SIGINFO`: The handler uses the three-parameter signature.
                sa_flags: libc::SA_SIGINFO,
                ..std::mem::zeroed()
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

        while current.park_count.load(Ordering::Relaxed) != 0 {
            current.park_count.fetch_sub(1, Ordering::Relaxed);

            // Park the current thread
            park_inner(current);
        }
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

#[derive(Copy, Clone)]
#[repr(transparent)]
struct JmpBuf {
    sp: NonNull<()>,
}

/// Call `cb`, preserving the current context state in `JmpBuf`, which
/// can be later used by [`longjmp`] to immediately return from this function,
/// bypassing destructors and unwinding mechanisms such as
/// <https://github.com/rust-lang/rust/pull/70212>.
///
/// [The native `setjmp`] isn't supported by Rust at the point of writing.
///
/// [The native `setjmp`]: https://github.com/rust-lang/rfcs/issues/2625
#[inline]
fn catch_longjmp<F: FnOnce(JmpBuf)>(cb: F) {
    #[inline(never)] // ensure all caller-saved regs are trash-able
    fn catch_longjmp_inner(f: fn(*mut (), JmpBuf), ctx: *mut ()) {
        unsafe {
            match () {
                #[cfg(target_arch = "x86_64")]
                () => {
                    asm!(
                        "
                            # push context
                            push rbp
                            push rbx
                            sub rsp, 8  # pad; ensure 16-byte stack alignment
                            lea rbx, [rip + 0f]
                            push rbx

                            # do f(ctx, jmp_buf)
                            # [rdi = ctx, rsp = jmp_buf]
                            mov rsi, rsp
                            call {f}

                            # discard context
                            add rsp, 32

                            jmp 1f
                        0:
                            # longjmp called. restore context
                            add rsp, 16  # skip 0b and the pad
                            pop rbx
                            pop rbp

                        1:
                        ",
                        f = inlateout(reg) f => _,
                        inlateout("rdi") ctx => _,
                        lateout("rsi") _,
                        // System V ABI callee-saved registers
                        // (note: Windows uses a different ABI)
                        lateout("r12") _,
                        lateout("r13") _,
                        lateout("r14") _,
                        lateout("r15") _,
                    );
                }

                #[cfg(target_arch = "aarch64")]
                () => {
                    asm!(
                        "
                            # push context. jump to 0 if longjmp is called
                            adr x2, 0f
                            sub sp, sp, #32
                            stp x2, x19, [sp]
                            stp x29, x30, [sp, #16]

                            # do f(ctx, jmp_buf)
                            # [x0 = ctx, x1 = jmp_buf]
                            mov x1, sp
                            blr {f}

                        0:
                            # restore x19, lr, and fp
                            ldp x29, x30, [sp, #16]
                            ldr x19, [sp, #8]

                            # discard context
                            add sp, sp, #32
                        ",
                        f = inlateout(reg) f => _,
                        inlateout("x0") ctx => _,
                        // AArch64 callee-saved registers
                        lateout("x20") _,
                        lateout("x21") _,
                        lateout("x22") _,
                        lateout("x23") _,
                        lateout("x24") _,
                        lateout("x25") _,
                        lateout("x26") _,
                        lateout("x27") _,
                        lateout("x28") _,
                        lateout("d8") _,
                        lateout("d9") _,
                        lateout("d10") _,
                        lateout("d11") _,
                        lateout("d12") _,
                        lateout("d13") _,
                        lateout("d14") _,
                        lateout("d15") _,
                    );
                }
            }
        }
    }

    let mut cb = core::mem::ManuallyDrop::new(cb);

    catch_longjmp_inner(
        |ctx, jmp_buf| unsafe {
            let ctx = (ctx as *mut F).read();
            ctx(jmp_buf);
        },
        (&mut cb) as *mut _ as *mut (),
    );
}

/// Return from a call to [`catch_longjmp`] using the preserved context state in
/// `jmp_buf`.
///
/// # Safety
///
///  - This function bypasses all destructor calls that stand between the call
///    site of this function and the call to `catch_longjmp` corresponding to
///    the given `JmpBuf`.
///
///  - The call to `catch_longjmp` corresponding to the given `JmpBuf` should be
///    still active (it must be in the call stack when this function is called).
///
unsafe fn longjmp(jmp_buf: JmpBuf) -> ! {
    unsafe {
        match () {
            #[cfg(target_arch = "x86_64")]
            () => {
                asm!(
                    "
                        mov rsp, {}
                        jmp [rsp]
                    ",
                    in(reg) jmp_buf.sp.as_ptr(),
                    options(noreturn),
                );
            }

            #[cfg(target_arch = "aarch64")]
            () => {
                asm!(
                    "
                        mov sp, {}
                        ldr x0, [sp, #0]
                        br x0
                    ",
                    in(reg) jmp_buf.sp.as_ptr(),
                    options(noreturn),
                );
            }
        }
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

    struct PanicOnDrop;

    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            unreachable!();
        }
    }

    #[test]
    fn test_longjmp() {
        let mut buf = 42;
        catch_longjmp(|jmp_buf| {
            let _hoge = PanicOnDrop;
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| loop {
                buf += 1;
                if buf == 50 {
                    unsafe { longjmp(jmp_buf) };
                }
            }))
            .unwrap();
        });
        assert_eq!(buf, 50);
    }
}
