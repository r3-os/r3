//! ~~Mutices~~ Mutexes
use core::{assert_matches::debug_assert_matches, fmt};
use r3_core::{
    kernel::{
        raw, LockMutexError, LockMutexTimeoutError, MarkConsistentMutexError, QueryMutexError,
        TryLockMutexError, UnlockMutexError,
    },
    time::Duration,
    utils::Init,
};

use crate::{
    error::{LockMutexPrecheckError, NoAccessError},
    klock, state, task, timeout,
    wait::{WaitPayload, WaitQueue},
    Id, KernelCfg1, KernelTraits, PortThreading, System,
};

pub(super) type MutexId = Id;

impl<Traits: KernelTraits> System<Traits> {
    /// Get the [`MutexCb`] for the specified raw ID.
    ///
    /// # Safety
    ///
    /// See [`crate::bad_id`].
    #[inline]
    unsafe fn mutex_cb(this: MutexId) -> Result<&'static MutexCb<Traits>, NoAccessError> {
        Traits::get_mutex_cb(this.get() - 1).ok_or_else(|| unsafe { crate::bad_id::<Traits>() })
    }
}

unsafe impl<Traits: KernelTraits> raw::KernelMutex for System<Traits> {
    type RawMutexId = MutexId;

    const RAW_SUPPORTED_MUTEX_PROTOCOLS: &'static [Option<raw::MutexProtocolKind>] = &[
        Some(raw::MutexProtocolKind::None),
        Some(raw::MutexProtocolKind::Ceiling),
    ];

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_mutex_is_locked(this: MutexId) -> Result<bool, QueryMutexError> {
        let lock = klock::lock_cpu::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let mutex_cb = unsafe { Self::mutex_cb(this)? };
        Ok(mutex_cb.owning_task.get(&*lock).is_some())
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_mutex_unlock(this: MutexId) -> Result<(), UnlockMutexError> {
        let lock = klock::lock_cpu::<Traits>()?;
        state::expect_waitable_context::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let mutex_cb = unsafe { Self::mutex_cb(this)? };

        unlock_mutex(mutex_cb, lock)
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_mutex_lock(this: MutexId) -> Result<(), LockMutexError> {
        let lock = klock::lock_cpu::<Traits>()?;
        state::expect_waitable_context::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let mutex_cb = unsafe { Self::mutex_cb(this)? };

        lock_mutex(mutex_cb, lock)
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_mutex_lock_timeout(
        this: MutexId,
        timeout: Duration,
    ) -> Result<(), LockMutexTimeoutError> {
        let time32 = timeout::time32_from_duration(timeout)?;
        let lock = klock::lock_cpu::<Traits>()?;
        state::expect_waitable_context::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let mutex_cb = unsafe { Self::mutex_cb(this)? };

        lock_mutex_timeout(mutex_cb, lock, time32)
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_mutex_try_lock(this: MutexId) -> Result<(), TryLockMutexError> {
        let lock = klock::lock_cpu::<Traits>()?;
        state::expect_task_context::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let mutex_cb = unsafe { Self::mutex_cb(this)? };

        try_lock_mutex(mutex_cb, lock)
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_mutex_mark_consistent(this: MutexId) -> Result<(), MarkConsistentMutexError> {
        let mut lock = klock::lock_cpu::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let mutex_cb = unsafe { Self::mutex_cb(this)? };

        if mutex_cb.inconsistent.replace(&mut *lock, false) {
            Ok(())
        } else {
            Err(MarkConsistentMutexError::BadObjectState)
        }
    }
}

/// *Mutex control block* - the state data of a mutex.
#[doc(hidden)]
pub struct MutexCb<
    Traits: PortThreading,
    TaskPriority: 'static = <Traits as KernelCfg1>::TaskPriority,
> {
    pub(super) ceiling: Option<TaskPriority>,

    pub(super) inconsistent: klock::CpuLockCell<Traits, bool>,

    pub(super) wait_queue: WaitQueue<Traits>,

    /// The next element in the singly-linked list headed by
    /// `TaskCb::last_mutex_held`, containing all mutexes currently held by the
    /// task.
    pub(super) prev_mutex_held: klock::CpuLockCell<Traits, Option<&'static Self>>,

    /// The task that currently owns the mutex lock.
    pub(super) owning_task: klock::CpuLockCell<Traits, Option<&'static task::TaskCb<Traits>>>,
}

impl<Traits: PortThreading> Init for MutexCb<Traits> {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self {
        ceiling: Init::INIT,
        inconsistent: Init::INIT,
        wait_queue: Init::INIT,
        prev_mutex_held: Init::INIT,
        owning_task: Init::INIT,
    };
}

impl<Traits: KernelTraits> fmt::Debug for MutexCb<Traits> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MutexCb")
            .field("self", &(self as *const _))
            .field("ceiling", &self.ceiling)
            .field("inconsistent", &self.inconsistent)
            .field("wait_queue", &self.wait_queue)
            .field(
                "prev_mutex_held",
                // prevent O((# of held mutexes)²)-order debug printing
                &self
                    .prev_mutex_held
                    .debug_fmt_with(|x, f| x.map(|x| x as *const _).fmt(f)),
            )
            .field(
                "owning_task",
                // break infinite recursion (TaskCb → MutxeCb → TaskCb → ...)
                &self
                    .owning_task
                    .debug_fmt_with(|x, f| x.map(|x| x as *const _).fmt(f)),
            )
            .finish()
    }
}

/// Check the error conditions covered by [`LockMutexPrecheckError`].
///
///  - `WouldDeadlock`: The current task already owns the mutex.
///
///  - `BadParam`: The mutex was created with the protocol attribute having the
///    value `Ceiling` and the current task's priority is higher than the
///    mutex's priority ceiling.
///
/// Returns the currently running task for convenience of the caller.
#[inline]
fn precheck_and_get_running_task<Traits: KernelTraits>(
    mut lock: klock::CpuLockTokenRefMut<'_, Traits>,
    mutex_cb: &'static MutexCb<Traits>,
) -> Result<&'static task::TaskCb<Traits>, LockMutexPrecheckError> {
    let task = Traits::state().running_task(lock.borrow_mut()).unwrap();

    if ptr_from_option_ref(mutex_cb.owning_task.get(&*lock)) == task {
        return Err(LockMutexPrecheckError::WouldDeadlock);
    }

    if let Some(ceiling) = mutex_cb.ceiling {
        if ceiling > task.base_priority.get(&*lock) {
            return Err(LockMutexPrecheckError::BadParam);
        }
    }

    Ok(task)
}

/// Check if the specified mutex, which is currently held or waited by a task,
/// is compatible with the new task base priority according to the mutex's
/// locking protocol.
///
/// The check is only needed when raising the priority.
#[inline]
pub(super) fn does_held_mutex_allow_new_task_base_priority<Traits: KernelTraits>(
    _lock: klock::CpuLockTokenRefMut<'_, Traits>,
    mutex_cb: &'static MutexCb<Traits>,
    new_base_priority: Traits::TaskPriority,
) -> bool {
    if let Some(ceiling) = mutex_cb.ceiling {
        if ceiling > new_base_priority {
            return false;
        }
    }

    true
}

/// Check if the task's held mutexes are all compatible with the new task base
/// priority according to the mutxes's locking protocols.
///
/// The check is only needed when raising the priority.
#[inline]
pub(super) fn do_held_mutexes_allow_new_task_base_priority<Traits: KernelTraits>(
    mut lock: klock::CpuLockTokenRefMut<'_, Traits>,
    task: &'static task::TaskCb<Traits>,
    new_base_priority: Traits::TaskPriority,
) -> bool {
    let mut maybe_mutex_cb = task.last_mutex_held.get(&*lock);
    while let Some(mutex_cb) = maybe_mutex_cb {
        if !does_held_mutex_allow_new_task_base_priority(
            lock.borrow_mut(),
            mutex_cb,
            new_base_priority,
        ) {
            return false;
        }

        maybe_mutex_cb = mutex_cb.prev_mutex_held.get(&*lock);
    }
    true
}

/// Reevaluate the task's effective priority and return the result.
/// (This method doesn't update [`task::TaskCb::effective_priority`]).
/// The base priority is assumed to be `base_priority`.
pub(super) fn evaluate_task_effective_priority<Traits: KernelTraits>(
    lock: klock::CpuLockTokenRefMut<'_, Traits>,
    task: &'static task::TaskCb<Traits>,
    base_priority: Traits::TaskPriority,
) -> Traits::TaskPriority {
    let mut effective_priority = base_priority;
    let mut maybe_mutex_cb = task.last_mutex_held.get(&*lock);

    while let Some(mutex_cb) = maybe_mutex_cb {
        if let Some(ceiling) = mutex_cb.ceiling {
            effective_priority = effective_priority.min(ceiling);
        }

        maybe_mutex_cb = mutex_cb.prev_mutex_held.get(&*lock);
    }

    effective_priority
}

/// Check if the current state of a mutex satisfies the wait
/// condition.
///
/// If it satisfies the wait condition, this function updates it and
/// returns `true`. Otherwise, it returns `false`, indicating the calling task
/// should be blocked.
#[inline]
fn poll_core<Traits: KernelTraits>(
    mutex_cb: &'static MutexCb<Traits>,
    running_task: &'static task::TaskCb<Traits>,
    lock: klock::CpuLockTokenRefMut<'_, Traits>,
) -> bool {
    if mutex_cb.owning_task.get(&*lock).is_some() {
        false
    } else {
        lock_core(mutex_cb, running_task, lock);
        true
    }
}

/// Give the ownership of the mutex to `task`.
///
/// The task must be in Running or Waiting state.
#[inline]
// FIXME: The extra parentheses in `debug_assert_matches!` are needed until
//        <https://github.com/murarth/assert_matches/pull/10> is fixed
#[allow(unused_parens)]
fn lock_core<Traits: KernelTraits>(
    mutex_cb: &'static MutexCb<Traits>,
    task: &'static task::TaskCb<Traits>,
    mut lock: klock::CpuLockTokenRefMut<'_, Traits>,
) {
    debug_assert_matches!(
        task.st.read(&*lock),
        (task::TaskSt::Running | task::TaskSt::Waiting)
    );

    mutex_cb.owning_task.replace(&mut *lock, Some(task));

    // Push `mutex_cb` to the list of the mutexes held by the task.
    let prev_mutex_held = task.last_mutex_held.replace(&mut *lock, Some(mutex_cb));
    mutex_cb
        .prev_mutex_held
        .replace(&mut *lock, prev_mutex_held);

    if let Some(ceiling) = mutex_cb.ceiling {
        let effective_priority = task.effective_priority.write(&mut *lock);
        *effective_priority = (*effective_priority).min(ceiling);
    }
}

#[inline]
fn lock_mutex<Traits: KernelTraits>(
    mutex_cb: &'static MutexCb<Traits>,
    mut lock: klock::CpuLockGuard<Traits>,
) -> Result<(), LockMutexError> {
    let running_task = precheck_and_get_running_task(lock.borrow_mut(), mutex_cb)?;

    if !poll_core(mutex_cb, running_task, lock.borrow_mut()) {
        // The current state does not satify the wait condition. In this case,
        // start waiting. The wake-upper is responsible for using `poll_core`
        // to complete the effect of the wait operation.
        mutex_cb
            .wait_queue
            .wait(lock.borrow_mut(), WaitPayload::Mutex(mutex_cb))?;
    }

    if mutex_cb.inconsistent.get(&*lock) {
        Err(LockMutexError::Abandoned)
    } else {
        Ok(())
    }
}

#[inline]
fn try_lock_mutex<Traits: KernelTraits>(
    mutex_cb: &'static MutexCb<Traits>,
    mut lock: klock::CpuLockGuard<Traits>,
) -> Result<(), TryLockMutexError> {
    let running_task = precheck_and_get_running_task(lock.borrow_mut(), mutex_cb)?;

    if !poll_core(mutex_cb, running_task, lock.borrow_mut()) {
        return Err(TryLockMutexError::Timeout);
    }

    if mutex_cb.inconsistent.get(&*lock) {
        Err(TryLockMutexError::Abandoned)
    } else {
        Ok(())
    }
}

#[inline]
fn lock_mutex_timeout<Traits: KernelTraits>(
    mutex_cb: &'static MutexCb<Traits>,
    mut lock: klock::CpuLockGuard<Traits>,
    time32: timeout::Time32,
) -> Result<(), LockMutexTimeoutError> {
    let running_task = precheck_and_get_running_task(lock.borrow_mut(), mutex_cb)?;

    if !poll_core(mutex_cb, running_task, lock.borrow_mut()) {
        // The current state does not satify the wait condition. In this case,
        // start waiting. The wake-upper is responsible for using `poll_core`
        // to complete the effect of the wait operation.
        mutex_cb.wait_queue.wait_timeout(
            lock.borrow_mut(),
            WaitPayload::Mutex(mutex_cb),
            time32,
        )?;
    }

    if mutex_cb.inconsistent.get(&*lock) {
        Err(LockMutexTimeoutError::Abandoned)
    } else {
        Ok(())
    }
}

#[inline]
fn unlock_mutex<Traits: KernelTraits>(
    mutex_cb: &'static MutexCb<Traits>,
    mut lock: klock::CpuLockGuard<Traits>,
) -> Result<(), UnlockMutexError> {
    let task = Traits::state().running_task(lock.borrow_mut()).unwrap();

    if ptr_from_option_ref(mutex_cb.owning_task.get(&*lock)) != task {
        // The current task does not currently own the mutex.
        return Err(UnlockMutexError::NotOwner);
    }

    if ptr_from_option_ref(task.last_mutex_held.get(&*lock)) != mutex_cb {
        // The correct mutex unlocking order is violated.
        return Err(UnlockMutexError::BadObjectState);
    }

    // Remove `mutex_cb` from the list of the mutexes held by the task.
    let prev_mutex_held = mutex_cb.prev_mutex_held.get(&*lock);
    task.last_mutex_held.replace(&mut *lock, prev_mutex_held);

    // Lower the task's effective priority. This may cause preemption.
    let base_priority = task.base_priority.get(&*lock);
    let effective_priority =
        evaluate_task_effective_priority(lock.borrow_mut(), task, base_priority);
    task.effective_priority
        .replace(&mut *lock, effective_priority);

    // Wake up the next waiter
    unlock_mutex_unchecked(mutex_cb, lock.borrow_mut());

    task::unlock_cpu_and_check_preemption(lock);

    Ok(())
}

/// Abandoon all mutexes held by the task.
///
/// This method doesn't restore the task's effective priority.
///
/// This method may make a task Ready, but doesn't yield the processor.
/// Call `unlock_cpu_and_check_preemption` (or something similar) as needed.
pub(super) fn abandon_held_mutexes<Traits: KernelTraits>(
    mut lock: klock::CpuLockTokenRefMut<'_, Traits>,
    task: &'static task::TaskCb<Traits>,
) {
    let mut maybe_mutex_cb = task.last_mutex_held.replace(&mut *lock, None);
    while let Some(mutex_cb) = maybe_mutex_cb {
        maybe_mutex_cb = mutex_cb.prev_mutex_held.get(&*lock);
        mutex_cb.inconsistent.replace(&mut *lock, true);
        unlock_mutex_unchecked(mutex_cb, lock.borrow_mut());
    }
}

/// Wake up the next waiter of the mutex.
///
/// This method doesn't restore the task's effective priority.
///
/// This method may make a task Ready, but doesn't yield the processor.
/// Call `unlock_cpu_and_check_preemption` (or something similar) if it returns
/// `true`.
fn unlock_mutex_unchecked<Traits: KernelTraits>(
    mutex_cb: &'static MutexCb<Traits>,
    mut lock: klock::CpuLockTokenRefMut<'_, Traits>,
) {
    // Check if there's any other tasks waiting on the mutex
    if let Some(next_task) = mutex_cb.wait_queue.first_waiting_task(lock.borrow_mut()) {
        // Give the ownership of the mutex to `next_task`
        lock_core(mutex_cb, next_task, lock.borrow_mut());

        // Wake up the next waiter
        assert!(mutex_cb.wait_queue.wake_up_one(lock.borrow_mut()));
    } else {
        // There's no one waiting
        mutex_cb.owning_task.replace(&mut *lock, None);
    }
}

#[inline]
fn ptr_from_option_ref<T>(x: Option<&T>) -> *const T {
    if let Some(x) = x {
        x
    } else {
        core::ptr::null()
    }
}
