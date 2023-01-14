//! Utterly inefficient cross-platform preemptive user-mode scheduling
use slab::Slab;
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::OnceLock,
    sync::{mpsc, Arc},
    thread::Result,
};

use crate::threading;

type SlabPtr = usize;

#[cfg(test)]
mod tests;

/// Represents a dynamic set of threads that can be scheduled for execution by
/// `Sched: `[`Scheduler`].
#[derive(Debug)]
pub struct ThreadGroup<Sched: ?Sized> {
    state: Arc<threading::Mutex<State<Sched>>>,
}

impl<Sched: ?Sized> Clone for ThreadGroup<Sched> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

/// Object that can be used to join on a [`ThreadGroup`].
#[derive(Debug)]
pub struct ThreadGroupJoinHandle {
    result_recv: mpsc::Receiver<Result<()>>,
}

/// RAII guard returned by [`ThreadGroup::lock`].
pub struct ThreadGroupLockGuard<'a, Sched: ?Sized> {
    state_ref: &'a Arc<threading::Mutex<State<Sched>>>,
    guard: threading::MutexGuard<'a, State<Sched>>,
}

/// Identifies a thread in [`ThreadGroup`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct ThreadId(SlabPtr);

/// Encapsulates the state of a client-supplied user-mode scheduler.
pub trait Scheduler: Send + 'static {
    /// Choose the next thread to run.
    ///
    /// It's an error to return an already-exited thread. The client is
    /// responsible for tracking the lifetime of spawned threads.
    fn choose_next_thread(&mut self) -> Option<ThreadId>;

    /// Called when a thread exits.
    fn thread_exited(&mut self, thread_id: ThreadId) {
        let _ = thread_id;
    }
}

#[derive(Debug)]
struct State<Sched: ?Sized> {
    threads: Slab<WorkerThread>,
    num_threads: usize,
    cur_thread_id: Option<ThreadId>,
    shutting_down: bool,
    result_send: mpsc::Sender<Result<()>>,
    sched: Sched,
}

#[derive(Debug)]
struct WorkerThread {
    join_handle: Option<threading::JoinHandle<()>>,
}

thread_local! {
    static TLB: OnceLock<ThreadLocalBlock> = OnceLock::new();
}

struct ThreadLocalBlock {
    /// The current thread ID.
    thread_id: ThreadId,
    /// The thread group the current worker thread belongs to.
    state: Arc<threading::Mutex<State<dyn Scheduler>>>,
}

impl<Sched: Scheduler> ThreadGroup<Sched> {
    /// Construct a new `ThreadGroup` and the corresponding
    /// [`ThreadGroupJoinHandle`].
    pub fn new(sched: Sched) -> (Self, ThreadGroupJoinHandle) {
        let (send, recv) = mpsc::channel();

        let state = Arc::new(threading::Mutex::new(State {
            threads: Slab::new(),
            num_threads: 0,
            cur_thread_id: None,
            shutting_down: false,
            result_send: send,
            sched,
        }));

        (Self { state }, ThreadGroupJoinHandle { result_recv: recv })
    }
}

impl ThreadGroupJoinHandle {
    /// Wait for the thread group to shut down.
    pub fn join(self) -> Result<()> {
        self.result_recv.recv().unwrap()
    }
}

impl<Sched: Scheduler + ?Sized> ThreadGroup<Sched> {
    /// Acquire a lock on the thread group's state.
    pub fn lock(&self) -> ThreadGroupLockGuard<'_, Sched> {
        ThreadGroupLockGuard {
            state_ref: &self.state,
            guard: self.state.lock().unwrap(),
        }
    }
}

impl<'a, Sched: Scheduler> ThreadGroupLockGuard<'a, Sched> {
    /// Start a worker thread.
    ///
    /// This does not automatically schedule the spawned thread. You should
    /// store the obtained `ThreadId` in the contained `Sched: `[`Scheduler`]
    /// and have it chosen by [`Scheduler::choose_next_thread`] for the thread
    /// to actually run.
    ///
    /// The following functions are avabile for use inside a worker thread. You
    /// should use them instead of the same named methods defined in other
    /// places.
    ///
    ///  - [`exit_thread`]
    ///  - [`yield_now`]
    ///
    pub fn spawn(&mut self, f: impl FnOnce(ThreadId) + Send + 'static) -> ThreadId {
        if self.guard.shutting_down && self.guard.num_threads == 0 {
            panic!("thread group has already been shut down");
        }

        let state = Arc::clone(self.state_ref);

        // Allocate a `ThreadId`
        let ptr: SlabPtr = self
            .guard
            .threads
            .insert(WorkerThread { join_handle: None });
        let thread_id = ThreadId(ptr);
        self.guard.num_threads += 1;

        let join_handle = threading::spawn(move || {
            let state2 = Arc::clone(&state);
            TLB.with(|cell| {
                cell.set(ThreadLocalBlock { thread_id, state })
                    .ok()
                    .unwrap()
            });

            // Block thw spawned thread until scheduled to run
            threading::park();

            // Call the thread entry point
            let result = catch_unwind(AssertUnwindSafe(move || {
                f(thread_id);
            }));

            finalize_thread(state2, thread_id, result);
        });

        // Save the `JoinHandle` representing the spawned thread
        self.guard.threads[ptr].join_handle = Some(join_handle);

        log::trace!("created {thread_id:?}");

        thread_id
    }

    /// Preempt the thread group to let the scheduler decide the next thread
    /// to run.
    ///
    /// Calling this method from a worker thread is not allowed.
    pub fn preempt(&mut self) {
        assert!(
            TLB.with(|cell| cell.get().is_none()),
            "this method cannot be called from a worker thread"
        );

        // Preeempt the current thread
        let guard = &mut *self.guard;
        log::trace!("preempting {:?}", guard.cur_thread_id);
        if let Some(thread_id) = guard.cur_thread_id {
            let join_handle = guard.threads[thread_id.0].join_handle.as_ref().unwrap();
            join_handle.thread().park();
        }

        guard.unpark_next_thread();
    }

    /// Initiate graceful shutdown for the thread group.
    ///
    /// The shutdown completes when all threads complete execution. After this
    /// happens, the system will not call [`Scheduler::choose_next_thread`]
    /// anymore. [`ThreadGroupJoinHandle::join`] will unblock, returning
    /// `Ok(())`.
    pub fn shutdown(&mut self) {
        if self.guard.shutting_down {
            return;
        }
        log::trace!("shutdown requested");
        self.guard.shutting_down = true;
        if self.guard.num_threads == 0 {
            self.guard.complete_shutdown();
        } else {
            log::trace!(
                "shutdown is pending because there are {} thread(s) remaining",
                self.guard.num_threads
            );
        }
    }
}

impl<'a, Sched: Scheduler + ?Sized> ThreadGroupLockGuard<'a, Sched> {
    /// Get a mutable reference to the contained `Sched: `[`Scheduler`].
    pub fn scheduler(&mut self) -> &mut Sched {
        &mut self.guard.sched
    }
}

impl<Sched: Scheduler> State<Sched> {
    fn unpark_next_thread(&mut self) {
        (self as &mut State<dyn Scheduler>).unpark_next_thread();
    }

    fn complete_shutdown(&mut self) {
        (self as &mut State<dyn Scheduler>).complete_shutdown();
    }
}

impl State<dyn Scheduler> {
    /// Find the next thread to run and unpark that thread.
    fn unpark_next_thread(&mut self) {
        self.cur_thread_id = self.sched.choose_next_thread();
        log::trace!("scheduling {:?}", self.cur_thread_id);
        if let Some(thread_id) = self.cur_thread_id {
            let join_handle = self.threads[thread_id.0].join_handle.as_ref().unwrap();
            join_handle.thread().unpark();
        }
    }

    fn complete_shutdown(&mut self) {
        assert_eq!(self.num_threads, 0);
        log::trace!("shutdown is complete");

        // Ignore if the receiver has already hung up
        let _ = self.result_send.send(Ok(()));
    }
}

/// Voluntarily yield the processor to let the scheduler decide the next thread
/// to run.
///
/// Panics if the current thread is not a worker thread of some [`ThreadGroup`].
pub fn yield_now() {
    let thread_group: Arc<threading::Mutex<State<dyn Scheduler>>> = TLB
        .with(|cell| cell.get().map(|tlb| Arc::clone(&tlb.state)))
        .expect("current thread does not belong to a thread group");

    {
        let mut state_guard = thread_group.lock().unwrap();
        log::trace!("{:?} yielded the processor", state_guard.cur_thread_id);
        state_guard.unpark_next_thread();
    }

    // Block thw thread until scheduled to run. This might end immediately if
    // the current thread is the next thread to run.
    threading::park();
}

/// Terminate the current worker thread.
///
/// Panics if the current thread is not a worker thread of some [`ThreadGroup`].
///
/// # Safety
///
/// It comes with all the unsafety of terminating a thread, such as that it
/// could unpin pinned local variables.
pub unsafe fn exit_thread() -> ! {
    let (thread_id, thread_group) = TLB
        .with(|cell| {
            cell.get()
                .map(|tlb| (tlb.thread_id, Arc::clone(&tlb.state)))
        })
        .expect("current thread does not belong to a thread group");

    finalize_thread(thread_group, thread_id, Ok(()));

    // Safety: Inherited
    unsafe { threading::exit_thread() };
}

/// Mark the specified thread as exited.
fn finalize_thread(
    thread_group: Arc<threading::Mutex<State<dyn Scheduler>>>,
    thread_id: ThreadId,
    result: Result<()>,
) {
    log::trace!("{thread_id:?} exited with result {result:?}");

    // Delete the current thread
    let mut state_guard = thread_group.lock().unwrap();
    state_guard.sched.thread_exited(thread_id);
    state_guard.threads.remove(thread_id.0);
    state_guard.num_threads -= 1;

    if let Err(e) = result {
        // Send the panic payload to the thread group's owner.
        // Leave other threads hanging because there's no way to
        // terminate them safely.
        // This should be at least sufficient for running tests and
        // apps with `panic = "abort"`.
        let _ = state_guard.result_send.send(Err(e));
        return;
    }

    if state_guard.num_threads == 0 && state_guard.shutting_down {
        // Complete the shutdown
        state_guard.complete_shutdown();
        return;
    }

    // Invoke the scheduler
    state_guard.unpark_next_thread();
}

/// Get the current worker thread.
pub fn current_thread() -> Option<ThreadId> {
    TLB.with(|cell| cell.get().map(|tlb| tlb.thread_id))
}
