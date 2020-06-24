use core::{fmt, ops, ptr::NonNull};

use super::{
    event_group, task,
    task::{TaskCb, TaskSt},
    utils::{CpuLockCell, CpuLockGuardBorrowMut},
    BadObjectStateError, Kernel, Port, WaitError,
};

use crate::utils::{
    intrusive_list::{self, ListAccessorCell},
    Init,
};

// Type definitions and trait implementations for wait lists
// ---------------------------------------------------------------------------

/// A reference to a [`Wait`].
struct WaitRef<System: Port>(NonNull<Wait<System>>);

// Safety: `Wait` is `Send + Sync`
unsafe impl<System: Port> Send for WaitRef<System> {}
unsafe impl<System: Port> Sync for WaitRef<System> {}

impl<System: Port> Clone for WaitRef<System> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<System: Port> Copy for WaitRef<System> {}

impl<System: Port> fmt::Debug for WaitRef<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("WaitRef").field(&self.0).finish()
    }
}

impl<System: Port> PartialEq for WaitRef<System> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<System: Port> Eq for WaitRef<System> {}

use self::unsafe_static::UnsafeStatic;
mod unsafe_static {
    use super::*;

    pub struct UnsafeStatic {
        _nonexhaustive: (),
    }

    impl UnsafeStatic {
        /// Construct an `UnsafeStatic`.
        ///
        /// # Safety
        ///
        /// All pointees to be accessed through the constructed `UnsafeStatic`
        /// must be valid.
        pub const unsafe fn new() -> &'static Self {
            &Self { _nonexhaustive: () }
        }
    }

    impl<System: Port> ops::Index<WaitRef<System>> for UnsafeStatic {
        type Output = Wait<System>;

        fn index(&self, index: WaitRef<System>) -> &Self::Output {
            // Safety: See `wait_queue_accessor`.
            unsafe { &*index.0.as_ptr() }
        }
    }
}

/// Get a `ListAccessorCell` used to access a wait queue.
///
/// # Safety
///
/// All elements of `$list` must be extant.
macro_rules! wait_queue_accessor {
    ($list:expr, $key:expr) => {
        ListAccessorCell::new(
            $list,
            UnsafeStatic::new(),
            |wait: &Wait<_>| &wait.link,
            $key,
        )
    };
}

// ---------------------------------------------------------------------------

/// *A wait object* describing *which task* is waiting on *what condition*.
///
/// # Lifetime
///
/// This object is constructed by `WaitQueue::wait` on a waiting task's stack,
/// and only survives until the method returns. This means that `Wait` can
/// expire only when the waiting task is not waiting anymore.
struct Wait<System: Port> {
    /// The task that is waiting for something.
    task: &'static TaskCb<System>,

    /// Forms a linked list headed by `wait_queue.waits`.
    link: CpuLockCell<System, Option<intrusive_list::Link<WaitRef<System>>>>,

    /// The containing [`WaitQueue`].
    wait_queue: Option<&'static WaitQueue<System>>,

    payload: WaitPayload<System>,
}

/// Additional information included in `With`, specific to waitable object
/// types.
pub(super) enum WaitPayload<System> {
    EventGroupBits {
        bits: event_group::EventGroupBits,
        flags: event_group::EventGroupWaitFlags,
        orig_bits: event_group::AtomicEventGroupBits,
    },
    Park,
    __Nonexhaustive(System),
}

/// A queue of wait objects ([`Wait`]) waiting on a particular waitable object.
pub(crate) struct WaitQueue<System: Port> {
    /// Wait objects waiting on the waitable object associated with this
    /// instance of `WaitQueue`. The waiting tasks (`Wait::task`) must be in a
    /// Waiting state.
    ///
    /// All elements of this linked list must be valid.
    waits: CpuLockCell<System, intrusive_list::ListHead<WaitRef<System>>>,

    order: QueueOrder,
}

impl<System: Port> Init for WaitQueue<System> {
    const INIT: Self = Self {
        waits: Init::INIT,
        order: QueueOrder::Fifo,
    };
}

/// Specifies the sorting order of a wait queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueOrder {
    /// The wait queue is processed in a FIFO order.
    Fifo,
    /// The wait queue is processed in a task priority order. Tasks with the
    /// same priorities follow a FIFO order.
    TaskPriority,
}

/// The wait state of a task.
pub(crate) struct TaskWait<System: Port> {
    /// The wait object describing the ongoing Waiting state of the task. Should
    /// be `None` iff the task is not in the Waiting state.
    ///
    /// The pointee must be valid.
    current_wait: CpuLockCell<System, Option<WaitRef<System>>>,

    /// The result of the last wait operation. Set by a wake-upper. Returned by
    /// [`WaitQueue::wait`].
    wait_result: CpuLockCell<System, Result<(), WaitError>>,
}

impl<System: Port> Init for TaskWait<System> {
    const INIT: Self = Self {
        current_wait: Init::INIT,
        wait_result: CpuLockCell::new(Ok(())),
    };
}

impl<System: Port> WaitQueue<System> {
    /// Construct a `WaitQueue`.
    pub(super) const fn new(order: QueueOrder) -> Self {
        Self {
            waits: Init::INIT,
            order,
        }
    }
}

impl<System: Kernel> WaitQueue<System> {
    /// Insert a wait object pertaining to the currently running task to `self`,
    /// transitioning the task into the Waiting state.
    ///
    /// The current context must be [waitable] (This function doesn't check
    /// that). The caller should use `expect_waitable_context` to do that.
    ///
    /// [waitable]: crate#contets
    #[inline]
    pub(super) fn wait(
        &'static self,
        lock: CpuLockGuardBorrowMut<'_, System>,
        payload: WaitPayload<System>,
    ) -> Result<WaitPayload<System>, WaitError> {
        let task = System::state().running_task().unwrap();
        let wait = Wait {
            task,
            link: CpuLockCell::new(None),
            wait_queue: Some(self),
            payload,
        };

        self.wait_inner(lock, &wait)?;

        Ok(wait.payload)
    }

    /// The core portion of `Self::wait`.
    ///
    /// Passing `WaitPayload` by value is expensive, so moving `WaitPayload`
    /// into and out of `Wait` is done in the outer function `Self::wait` with
    /// `#[inline]`.
    fn wait_inner(
        &'static self,
        mut lock: CpuLockGuardBorrowMut<'_, System>,
        wait: &Wait<System>,
    ) -> Result<(), WaitError> {
        let task = wait.task;
        let wait_ref = WaitRef(wait.into());

        debug_assert!(core::ptr::eq(
            wait.task,
            System::state().running_task().unwrap()
        ));
        debug_assert!(core::ptr::eq(wait.wait_queue.unwrap(), self));

        // Insert `wait_ref` into `self.waits`
        // Safety: All elements of `self.waits` are extant.
        let mut accessor = unsafe { wait_queue_accessor!(&self.waits, lock.borrow_mut()) };
        let insert_at = match self.order {
            QueueOrder::Fifo => {
                // FIFO order - insert at the back
                None
            }
            QueueOrder::TaskPriority => {
                let cur_task_pri = task.priority;
                let mut insert_at = None;
                let mut cursor = accessor.back();
                while let Some(next_cursor) = cursor {
                    // Should the new wait object inserted at this or an earlier
                    // position?
                    if accessor.pool()[next_cursor].task.priority > cur_task_pri {
                        // If so, update `insert_at`. Continue searching because
                        // there might be a viable position that is even
                        // earlier.
                        insert_at = Some(next_cursor);
                        cursor = accessor.prev(next_cursor);
                    } else {
                        break;
                    }
                }
                insert_at
            }
        };
        accessor.insert(wait_ref, insert_at);

        // Set `task.current_wait`
        task.wait.current_wait.replace(&mut *lock, Some(wait_ref));

        // Transition the task into Waiting. This statement will complete when
        // the task is woken up.
        task::wait_until_woken_up(lock.borrow_mut());

        // `wait_ref` should have been removed from a wait queue by a wake-upper
        assert!(wait.link.read(&*lock).is_none());
        assert!(task.wait.current_wait.get(&*lock).is_none());

        // Return the wait result (`Ok(())` or `Err(Interrupted)`)
        task.wait.wait_result.get(&*lock)
    }

    /// Wake up up to one waiting task. Returns `true` if it has successfully
    /// woken up a task.
    ///
    /// This method may make a task Ready, but doesn't yield the processor.
    /// Call `unlock_cpu_and_check_preemption` as needed.
    pub(super) fn wake_up_one(&self, mut lock: CpuLockGuardBorrowMut<'_, System>) -> bool {
        // Get the first wait object
        // Safety: All elements of `self.waits` are extant.
        let wait_ref = unsafe { wait_queue_accessor!(&self.waits, lock.borrow_mut()) }.pop_front();

        let wait_ref = if let Some(wait_ref) = wait_ref {
            wait_ref
        } else {
            return false;
        };

        // Safety: `wait_ref` points to a valid `Wait` because `wait_ref` was
        // in `self.waits` at the beginning of this function call.
        let wait = unsafe { wait_ref.0.as_ref() };

        assert!(core::ptr::eq(wait.wait_queue.unwrap(), self));

        complete_wait(lock.borrow_mut(), wait, Ok(()));

        true
    }

    /// Wake up all waiting tasks. Returns `true` if it has successfully
    /// woken up at least one task.
    ///
    /// This method may make a task Ready, but doesn't yield the processor.
    /// Call `unlock_cpu_and_check_preemption` as needed.
    pub(super) fn wake_up_all(&self, mut lock: CpuLockGuardBorrowMut<'_, System>) -> bool {
        // Call `wake_up_one` repeatedly until it returns `false`. If the first
        // call returns `true`, the result of `wake_up_all` is `true`.
        self.wake_up_one(lock.borrow_mut()) && {
            while self.wake_up_one(lock.borrow_mut()) {}
            true
        }
    }

    /// Conditionally wake up waiting tasks.
    ///
    /// This method may make a task Ready, but doesn't yield the processor.
    /// Call `unlock_cpu_and_check_preemption` as needed.
    pub(super) fn wake_up_all_conditional(
        &self,
        mut lock: CpuLockGuardBorrowMut<'_, System>,
        mut cond: impl FnMut(&WaitPayload<System>) -> bool,
    ) {
        // Safety: All elements of `self.waits` are extant.
        let mut cur = unsafe { wait_queue_accessor!(&self.waits, lock.borrow_mut()) }.front();

        while let Some(wait_ref) = cur {
            // Find the next wait object before we possibly remove `wait_ref`
            // from `self.waits`.
            // Safety: All elements of `self.waits` are extant.
            cur = unsafe { wait_queue_accessor!(&self.waits, lock.borrow_mut()) }.next(wait_ref);

            // Dereference `wait_ref` and get `&Wait`
            // Safety: `wait_ref` points to a valid `Wait` because `wait_ref` is
            // in `self.waits`.
            let wait = unsafe { wait_ref.0.as_ref() };

            assert!(core::ptr::eq(wait.wait_queue.unwrap(), self));

            // Should this task be woken up?
            if !cond(&wait.payload) {
                continue;
            }

            // Wake up the task
            // Safety: All elements of `self.waits` are extant.
            unsafe { wait_queue_accessor!(&self.waits, lock.borrow_mut()) }.remove(wait_ref);
            complete_wait(lock.borrow_mut(), wait, Ok(()));
        }
    }
}

/// Call the given closure with a reference to the current wait payload object
/// of the specified task as the closure's parameter.
///
/// The wait object might get deallocated when the task starts running. This
/// function allows access to the wait object while ensuring the reference to
/// the wait object doesn't escape from the scope.
pub(super) fn with_current_wait_payload<System: Kernel, R>(
    lock: CpuLockGuardBorrowMut<'_, System>,
    task_cb: &TaskCb<System>,
    f: impl FnOnce(Option<&WaitPayload<System>>) -> R,
) -> R {
    let wait_ref = task_cb.wait.current_wait.get(&*lock);

    // Safety: ... and `wait_ref` must point to an existing `Wait`
    let wait = wait_ref.map(|r| &unsafe { &*r.0.as_ptr() }.payload);

    f(wait)
}

/// Insert a wait object pertaining to the currently running task to `self` but
/// not pertainiing to any wait queue, transitioning the task into a Waiting
/// state.
///
/// The only way to end such a wait operation is to call [`interrupt_task`].
///
/// The current context must be [waitable] (This function doesn't check
/// that). The caller should use `expect_waitable_context` to do that.
///
/// [waitable]: crate#contets
#[inline]
pub(super) fn wait_no_queue<System: Kernel>(
    lock: CpuLockGuardBorrowMut<'_, System>,
    payload: WaitPayload<System>,
) -> Result<WaitPayload<System>, WaitError> {
    let task = System::state().running_task().unwrap();
    let wait = Wait {
        task,
        link: CpuLockCell::new(None),
        wait_queue: None,
        payload,
    };

    wait_no_queue_inner(lock, &wait)?;

    Ok(wait.payload)
}

/// The core portion of [`wait_no_queue`].
///
/// Passing `WaitPayload` by value is expensive, so moving `WaitPayload`
/// into and out of `Wait` is done in the outer function `Self::wait` with
/// `#[inline]`.
fn wait_no_queue_inner<System: Kernel>(
    mut lock: CpuLockGuardBorrowMut<'_, System>,
    wait: &Wait<System>,
) -> Result<(), WaitError> {
    let task = wait.task;
    let wait_ref = WaitRef(wait.into());

    debug_assert!(core::ptr::eq(
        wait.task,
        System::state().running_task().unwrap()
    ));
    debug_assert!(wait.wait_queue.is_none());
    debug_assert!(wait.link.read(&*lock).is_none());

    // Set `task.current_wait`
    task.wait.current_wait.replace(&mut *lock, Some(wait_ref));

    // Transition the task into Waiting. This statement will complete when
    // the task is woken up.
    task::wait_until_woken_up(lock.borrow_mut());

    // `wait_ref` should have been removed `current_wait` by a wake-upper
    assert!(task.wait.current_wait.get(&*lock).is_none());

    // Return the wait result (`Ok(())` or `Err(Interrupted)`)
    task.wait.wait_result.get(&*lock)
}

/// Deassociate the specified wait object from its waiting task (`wait.task`)
/// and wake up the task.
///
/// Panics if `wait` is not associated (anymore) with its waiting task.
///
/// This method doesn't remove `wait` from `WaitQueue:waits`.
///
/// This method may make a task Ready, but doesn't yield the processor.
/// Call `unlock_cpu_and_check_preemption` as needed.
fn complete_wait<System: Kernel>(
    mut lock: CpuLockGuardBorrowMut<'_, System>,
    wait: &Wait<System>,
    wait_result: Result<(), WaitError>,
) {
    let task_cb = wait.task;

    // Clear `TaskWait::current_wait`
    assert_eq!(
        *task_cb.wait.current_wait.read(&*lock),
        Some(WaitRef(wait.into()))
    );
    task_cb.wait.current_wait.replace(&mut *lock, None);

    // Set a wait result
    let _ = task_cb.wait.wait_result.replace(&mut *lock, wait_result);

    assert_eq!(*task_cb.st.read(&*lock), task::TaskSt::Waiting);

    // Make the task Ready
    //
    // Safety: The task is in the Waiting state, meaning the task state is valid
    // and ready to resume from the point where it was previously interrupted.
    // A proper clean up for exiting the Waiting state is already done as well.
    unsafe { task::make_ready(lock, task_cb) };
}

/// Interrupt any ongoing wait operations on the task.
///
/// This method may make the task Ready, but doesn't yield the processor.
/// Call `unlock_cpu_and_check_preemption` as needed.
///
/// Returns `Err(BadObjectState)` if the task is not in the Waiting state.
pub(super) fn interrupt_task<System: Kernel>(
    mut lock: CpuLockGuardBorrowMut<'_, System>,
    task_cb: &'static TaskCb<System>,
    wait_result: Result<(), WaitError>,
) -> Result<(), BadObjectStateError> {
    match *task_cb.st.read(&*lock) {
        TaskSt::Waiting => {
            // Interrupt the ongoing wait operation.
            let wait_ref = task_cb.wait.current_wait.get(&*lock);

            // The task is in the Waiting state, so `wait_ref` must be `Some(_)`
            let wait_ref = wait_ref.unwrap();

            // Safety: ... and `wait_ref` must point to an existing `Wait`
            let wait = unsafe { wait_ref.0.as_ref() };

            // Remove `wait` from the wait queue it belongs to
            if let Some(wait_queue) = wait.wait_queue {
                unsafe { wait_queue_accessor!(&wait_queue.waits, lock.borrow_mut()) }
                    .remove(wait_ref);
            }

            // Wake up the task
            complete_wait(lock.borrow_mut(), wait, wait_result);

            Ok(())
        }
        _ => Err(BadObjectStateError::BadObjectState),
    }
}
