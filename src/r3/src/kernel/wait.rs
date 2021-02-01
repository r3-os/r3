use core::{fmt, ops, ptr::NonNull};

use super::{
    event_group, mutex, task,
    task::{TaskCb, TaskSt},
    timeout,
    utils::{CpuLockCell, CpuLockGuard, CpuLockTokenRefMut},
    BadObjectStateError, Kernel, Port, PortThreading, WaitError, WaitTimeoutError,
};

use crate::utils::{
    intrusive_list::{self, HandleInconsistencyUnchecked, ListAccessorCell},
    Init,
};

// Type definitions and trait implementations for wait lists
// ---------------------------------------------------------------------------

/// A reference to a [`Wait`].
struct WaitRef<System: PortThreading>(NonNull<Wait<System>>);

// Safety: `Wait` is `Send + Sync`
unsafe impl<System: PortThreading> Send for WaitRef<System> {}
unsafe impl<System: PortThreading> Sync for WaitRef<System> {}

impl<System: PortThreading> Clone for WaitRef<System> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<System: PortThreading> Copy for WaitRef<System> {}

impl<System: PortThreading> fmt::Debug for WaitRef<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("WaitRef").field(&self.0).finish()
    }
}

impl<System: PortThreading> PartialEq for WaitRef<System> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<System: PortThreading> Eq for WaitRef<System> {}

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
        #[inline]
        pub const unsafe fn new() -> &'static Self {
            &Self { _nonexhaustive: () }
        }
    }

    impl<System: Port> ops::Index<WaitRef<System>> for UnsafeStatic {
        type Output = Wait<System>;

        #[inline]
        fn index(&self, index: WaitRef<System>) -> &Self::Output {
            // Safety: See `wait_queue_accessor`.
            unsafe { &*index.0.as_ptr() }
        }
    }
}

/// Get a `ListAccessorCell` used to access a wait queue.
macro_rules! wait_queue_accessor {
    ($list:expr, $key:expr) => {
        unsafe {
            ListAccessorCell::new(
                $list,
                // Safety: All elements are extant because we never drop a
                //     `Wait` when it's still in a wait queue.
                UnsafeStatic::new(),
                |wait: &Wait<_>| &wait.link,
                $key,
            )
            // Safety: This linked list is structurally sound.
            .unchecked()
        }
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
struct Wait<System: PortThreading> {
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
pub(super) enum WaitPayload<System: PortThreading> {
    EventGroupBits {
        bits: event_group::EventGroupBits,
        flags: event_group::EventGroupWaitFlags,
        orig_bits: event_group::AtomicEventGroupBits,
    },
    Semaphore,
    Mutex(&'static mutex::MutexCb<System>),
    Park,
    Sleep,
    __Nonexhaustive,
}

impl<T: PortThreading> WaitPayload<T> {
    /// Return `self`.
    ///
    /// This might look redundant but is actually very important to maximize
    /// performance of moving `WaitPayload`. Without this, the compiler would
    /// try very hard to preserve the bit pattern of the unused space within
    /// `payload` by using `memcpy`, which is extremely slow. I've seen a 30%
    /// decrease in the execution time in the `semaphore` benchmark as a result
    /// of using this method.
    #[inline]
    fn r#move(self) -> Self {
        match self {
            Self::EventGroupBits {
                bits,
                flags,
                orig_bits,
            } => Self::EventGroupBits {
                bits,
                flags,
                orig_bits,
            },
            Self::Semaphore => Self::Semaphore,
            Self::Mutex(x) => Self::Mutex(x),
            Self::Park => Self::Park,
            Self::Sleep => Self::Sleep,
            Self::__Nonexhaustive => Self::__Nonexhaustive,
        }
    }
}

/// A queue of wait objects ([`Wait`]) waiting on a particular waitable object.
pub(crate) struct WaitQueue<System: PortThreading> {
    /// Wait objects waiting on the waitable object associated with this
    /// instance of `WaitQueue`. The waiting tasks (`Wait::task`) must be in a
    /// Waiting state.
    ///
    /// All elements of this linked list must be valid.
    waits: CpuLockCell<System, intrusive_list::ListHead<WaitRef<System>>>,

    order: QueueOrder,
}

impl<System: PortThreading> Init for WaitQueue<System> {
    #[allow(clippy::declare_interior_mutable_const)]
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
pub(crate) struct TaskWait<System: PortThreading> {
    /// The wait object describing the ongoing Waiting state of the task. Should
    /// be `None` iff the task is not in the Waiting state.
    ///
    /// The pointee must be valid.
    current_wait: CpuLockCell<System, Option<WaitRef<System>>>,

    /// The result of the last wait operation. Set by a wake-upper. Returned by
    /// [`WaitQueue::wait`].
    wait_result: CpuLockCell<System, Result<(), WaitTimeoutError>>,
}

impl<System: PortThreading> Init for TaskWait<System> {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self {
        current_wait: Init::INIT,
        wait_result: CpuLockCell::new(Ok(())),
    };
}

/// Register a timeout object to interrupt `$task_cb` after the duration
/// specified by `$duration_time32`. The timeout object remains valid throughout
/// the current lexical scope.
///
/// This macro is used inside a blocking operation with timeout.
macro_rules! setup_timeout_wait {
    ($lock:ident, $task_cb:expr, $duration_time32:expr) => {
        // Create a timeout object.
        let timeout = new_timeout_object_for_task($lock.borrow_mut(), $task_cb, $duration_time32);
        pin_utils::pin_mut!(timeout);

        // Use `TimeoutGuard` to automatically unregister the timeout when
        // leaving the current lexical scope.
        let mut timeout_guard = timeout::TimeoutGuard {
            timeout: timeout.as_ref(),
            lock: $lock,
        };
        let mut $lock = timeout_guard.lock.borrow_mut();

        // Register the timeout object
        timeout::insert_timeout($lock.borrow_mut(), timeout_guard.timeout);
    };
}

impl<System: PortThreading> WaitQueue<System> {
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
    /// [waitable]: crate#contexts
    #[inline]
    pub(super) fn wait(
        &'static self,
        mut lock: CpuLockTokenRefMut<'_, System>,
        payload: WaitPayload<System>,
    ) -> Result<WaitPayload<System>, WaitError> {
        let task = System::state().running_task(lock.borrow_mut()).unwrap();
        let wait = Wait {
            task,
            link: CpuLockCell::new(None),
            wait_queue: Some(self),
            payload: payload.r#move(),
        };

        self.wait_inner(lock, &wait)
            .map_err(WaitTimeoutError::expect_not_timeout)?;

        Ok(wait.payload)
    }

    /// Insert a wait object pertaining to the currently running task to `self`,
    /// transitioning the task into the Waiting state. The operation will time
    /// out after the specified duration.
    ///
    /// The current context must be [waitable] (This function doesn't check
    /// that). The caller should use `expect_waitable_context` to do that.
    ///
    /// [waitable]: crate#contexts
    #[inline]
    pub(super) fn wait_timeout(
        &'static self,
        mut lock: CpuLockTokenRefMut<'_, System>,
        payload: WaitPayload<System>,
        duration_time32: timeout::Time32,
    ) -> Result<WaitPayload<System>, WaitTimeoutError> {
        let task = System::state().running_task(lock.borrow_mut()).unwrap();
        let wait = Wait {
            task,
            link: CpuLockCell::new(None),
            wait_queue: Some(self),
            payload: payload.r#move(),
        };

        // Configure a timeout
        setup_timeout_wait!(lock, task, duration_time32);

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
        mut lock: CpuLockTokenRefMut<'_, System>,
        wait: &Wait<System>,
    ) -> Result<(), WaitTimeoutError> {
        let task = wait.task;
        let wait_ref = WaitRef(wait.into());

        debug_assert!(core::ptr::eq(
            wait.task,
            System::state().running_task(lock.borrow_mut()).unwrap()
        ));
        debug_assert!(core::ptr::eq(wait.wait_queue.unwrap(), self));

        // Insert `wait_ref` into `self.waits`
        // Safety: All elements of `self.waits` are extant.
        let mut accessor = wait_queue_accessor!(&self.waits, lock.borrow_mut());
        let insert_at = match self.order {
            QueueOrder::Fifo => {
                // FIFO order - insert at the back
                None
            }
            QueueOrder::TaskPriority => {
                let cur_task_pri = *task.effective_priority.read(&**accessor.cell_key());
                // TODO: It's unfortunate that we need to pass
                //       `&ListAccessorCell`, which incurs a runtime cost because
                //       `&T` is always pointer-sized. Find a way to eliminate
                //       this runtime cost.
                Self::find_insertion_position_by_task_priority(cur_task_pri, &accessor)
            }
        };

        // Safety: `wait_ref` is not linked, so it shouldn't return
        //     `InsertError::AlreadyLinked`.
        unsafe { accessor.insert(wait_ref, insert_at).unwrap_unchecked() };

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

    /// Find the insertion position for a wait object owned by a task whose
    /// priority is `cur_task_pri`.
    fn find_insertion_position_by_task_priority<MapLink>(
        cur_task_pri: System::TaskPriority,
        accessor: &ListAccessorCell<
            '_,
            &CpuLockCell<System, intrusive_list::ListHead<WaitRef<System>>>,
            UnsafeStatic,
            MapLink,
            CpuLockTokenRefMut<'_, System>,
            HandleInconsistencyUnchecked,
        >,
    ) -> Option<WaitRef<System>>
    where
        MapLink: Fn(
            &Wait<System>,
        ) -> &CpuLockCell<System, Option<intrusive_list::Link<WaitRef<System>>>>,
    {
        let mut insert_at = None;
        let Ok(mut cursor) = accessor.back();
        while let Some(next_cursor) = cursor {
            // Should the new wait object inserted at this or an earlier
            // position?
            let next_cursor_task = accessor.pool()[next_cursor].task;
            let next_cursor_task_pri = *next_cursor_task
                .effective_priority
                .read(&**accessor.cell_key());
            if next_cursor_task_pri > cur_task_pri {
                // If so, update `insert_at`. Continue searching because
                // there might be a viable position that is even
                // earlier.
                insert_at = Some(next_cursor);
                // Safety: `next_cursor` is linked, so `prev` shouldn't return
                //         `ItemError::Unlinked`.
                cursor = unsafe { accessor.prev(next_cursor).unwrap_unchecked() };
            } else {
                break;
            }
        }
        insert_at
    }

    /// Reposition `wait` in the wait queue. This is necessary after
    /// changing the waiting task's priority.
    fn reorder_wait(&'static self, mut lock: CpuLockTokenRefMut<'_, System>, wait: &Wait<System>) {
        match self.order {
            QueueOrder::Fifo => return,
            QueueOrder::TaskPriority => {}
        }

        let wait_ref = WaitRef(wait.into());
        let task = wait.task;
        debug_assert!(core::ptr::eq(wait.wait_queue.unwrap(), self));

        // Safety: All elements of `self.waits` are extant.
        let mut accessor = wait_queue_accessor!(&self.waits, lock.borrow_mut());

        // Remove `wait_ref` first.
        //
        // Safety: `wait_ref` is linked, so it shouldn't return
        //     `ItemError::Unlinked`.
        unsafe {
            accessor.remove(wait_ref).unwrap_unchecked();
        }

        // Re-insert `wait_ref`.
        //
        // Safety: This linked list is structurally sound, so `insert` shouldn't
        // return `InconsistentError`. `wait_ref` is unlinked, so it shouldn't
        // return `InsertError::AlreadyLinked` either.
        let cur_task_pri = *task.effective_priority.read(&**accessor.cell_key());
        let insert_at = Self::find_insertion_position_by_task_priority(cur_task_pri, &accessor);
        unsafe {
            accessor.insert(wait_ref, insert_at).unwrap_unchecked();
        }
    }

    /// Get the next waiting task to be woken up.
    pub(super) fn first_waiting_task(
        &self,
        mut lock: CpuLockTokenRefMut<'_, System>,
    ) -> Option<&'static TaskCb<System>> {
        // Get the waiting task of the first wait object
        // Safety: This linked list is structurally sound, so it shouldn't
        //         return `Err(InconsistentError)`
        let accessor = wait_queue_accessor!(&self.waits, lock.borrow_mut());
        unsafe { accessor.front_data().unwrap_unchecked() }.map(|wait| wait.task)
    }

    /// Wake up up to one waiting task. Returns `true` if it has successfully
    /// woken up a task.
    ///
    /// This method may make a task Ready, but doesn't yield the processor.
    /// Call `unlock_cpu_and_check_preemption` as needed.
    pub(super) fn wake_up_one(&self, mut lock: CpuLockTokenRefMut<'_, System>) -> bool {
        // Get the first wait object
        // Safety: This linked list is structurally sound, so it shouldn't
        //         return `Err(InconsistentError)`
        let mut accessor = wait_queue_accessor!(&self.waits, lock.borrow_mut());
        let wait_ref = unsafe { accessor.pop_front().unwrap_unchecked() };

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

    /// Conditionally wake up waiting tasks.
    ///
    /// This method may make a task Ready, but doesn't yield the processor.
    /// Call `unlock_cpu_and_check_preemption` as needed.
    pub(super) fn wake_up_all_conditional(
        &self,
        mut lock: CpuLockTokenRefMut<'_, System>,
        mut cond: impl FnMut(&WaitPayload<System>) -> bool,
    ) {
        let Ok(mut cur) = {
            let accessor = wait_queue_accessor!(&self.waits, lock.borrow_mut());
            accessor.front()
        };

        while let Some(wait_ref) = cur {
            // Find the next wait object before we possibly remove `wait_ref`
            // from `self.waits`.
            cur = {
                let accessor = wait_queue_accessor!(&self.waits, lock.borrow_mut());
                // Safety: `wait_ref` is still linked, so it shouldn't return
                //         `ItemError::Unlinked`.
                unsafe { accessor.next(wait_ref).unwrap_unchecked() }
            };

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
            let mut accessor = wait_queue_accessor!(&self.waits, lock.borrow_mut());
            // Safety: `wait_ref` is still linked, so it shouldn't return
            //         `ItemError::Unlinked`.
            unsafe { accessor.remove(wait_ref).unwrap_unchecked() };

            complete_wait(lock.borrow_mut(), wait, Ok(()));
        }
    }
}

impl<System: Kernel> fmt::Debug for Wait<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{{ task: {:p}, payload: {:?} }}",
            self.task, self.payload
        )
    }
}

impl<System: Kernel> fmt::Debug for WaitPayload<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::EventGroupBits {
                bits,
                flags,
                orig_bits,
            } => f
                .debug_struct("EventGroupBits")
                .field("bits", bits)
                .field("flags", flags)
                .field("orig_bits", orig_bits)
                .finish(),
            Self::Semaphore => f.write_str("Semaphore"),
            Self::Mutex(mutex) => write!(f, "Mutex({:p})", mutex),
            Self::Park => f.write_str("Park"),
            Self::Sleep => f.write_str("Sleep"),
            Self::__Nonexhaustive => unreachable!(),
        }
    }
}

impl<System: Kernel> fmt::Debug for WaitQueue<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct WaitQueuePrinter<'a, System: Kernel> {
            waits: &'a CpuLockCell<System, intrusive_list::ListHead<WaitRef<System>>>,
        }

        impl<System: Kernel> fmt::Debug for WaitQueuePrinter<'_, System> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                if let Ok(mut lock) = super::utils::lock_cpu() {
                    let accessor = wait_queue_accessor!(&self.waits, lock.borrow_mut());

                    f.debug_list()
                        .entries(accessor.iter().map(|x| x.unwrap().1))
                        .finish()
                } else {
                    f.write_str("< locked >")
                }
            }
        }

        f.debug_struct("WaitQueue")
            .field("waits", &WaitQueuePrinter { waits: &self.waits })
            .field("order", &self.order)
            .finish()
    }
}

impl<System: Kernel> fmt::Debug for TaskWait<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TaskWait")
            .field(
                "current_wait",
                &self.current_wait.debug_fmt_with(|wait_ref, f| {
                    // Safety: ... and `wait_ref` must point to an existing `Wait`
                    let wait = wait_ref.map(|r| &unsafe { &*r.0.as_ptr() }.payload);
                    wait.fmt(f)
                }),
            )
            .field("wait_result", &self.wait_result)
            .finish()
    }
}

/// Access the specified task's current wait payload object in the supplied
/// closure.
///
/// The wait object might get deallocated when the task starts running. This
/// function allows access to the wait object while ensuring the reference to
/// the wait object doesn't escape from the scope.
pub(super) fn with_current_wait_payload<System: Kernel, R>(
    lock: CpuLockTokenRefMut<'_, System>,
    task_cb: &TaskCb<System>,
    f: impl FnOnce(Option<&WaitPayload<System>>) -> R,
) -> R {
    let wait_ref = task_cb.wait.current_wait.get(&*lock);

    // Safety: ... and `wait_ref` must point to an existing `Wait`
    let wait = wait_ref.map(|r| &unsafe { &*r.0.as_ptr() }.payload);

    f(wait)
}

/// Reposition the given task's wait object within the wait queue. This is
/// necessary after changing the task's priority because some wait queues are
/// configured to sort wait objects by task priority
/// ([`QueueOrder::TaskPriority`]).
///
/// This function does nothing if the task is currently not in the Waiting state
/// or the wait object is not associated with any wait queue.
pub(super) fn reorder_wait_of_task<System: Kernel>(
    lock: CpuLockTokenRefMut<'_, System>,
    task_cb: &TaskCb<System>,
) {
    if let Some(wait_ref) = task_cb.wait.current_wait.get(&*lock) {
        // Safety: `wait_ref` must point to an existing `Wait`
        let wait = unsafe { &*wait_ref.0.as_ptr() };

        if let Some(wait_queue) = wait.wait_queue {
            wait_queue.reorder_wait(lock, wait);
        }
    }
}

/// Create a wait object pertaining to the currently running task but
/// not pertaining to any wait queue. Transition the task into the Waiting
/// state.
///
/// The only way to end such a wait operation is to call [`interrupt_task`].
///
/// The current context must be [waitable] (This function doesn't check
/// that). The caller should use `expect_waitable_context` to do that.
///
/// [waitable]: crate#contexts
#[inline]
pub(super) fn wait_no_queue<System: Kernel>(
    mut lock: CpuLockTokenRefMut<'_, System>,
    payload: WaitPayload<System>,
) -> Result<WaitPayload<System>, WaitError> {
    let task = System::state().running_task(lock.borrow_mut()).unwrap();
    let wait = Wait {
        task,
        link: CpuLockCell::new(None),
        wait_queue: None,
        payload: payload.r#move(),
    };

    wait_no_queue_inner(lock, &wait).map_err(WaitTimeoutError::expect_not_timeout)?;

    Ok(wait.payload)
}

/// Create a wait object pertaining to the currently running task but
/// not pertaining to any wait queue. Transition the task into the Waiting
/// state. The operation will time out after the specified duration.
///
/// The only way to end such a wait operation is to call [`interrupt_task`] or
/// to wait until it times out.
///
/// The current context must be [waitable] (This function doesn't check
/// that). The caller should use `expect_waitable_context` to do that.
///
/// [waitable]: crate#contexts
#[inline]
pub(super) fn wait_no_queue_timeout<System: Kernel>(
    mut lock: CpuLockTokenRefMut<'_, System>,
    payload: WaitPayload<System>,
    duration_time32: timeout::Time32,
) -> Result<WaitPayload<System>, WaitTimeoutError> {
    let task = System::state().running_task(lock.borrow_mut()).unwrap();
    let wait = Wait {
        task,
        link: CpuLockCell::new(None),
        wait_queue: None,
        payload: payload.r#move(),
    };

    // Configure a timeout
    setup_timeout_wait!(lock, task, duration_time32);

    wait_no_queue_inner(lock, &wait)?;

    Ok(wait.payload)
}

/// The core portion of [`wait_no_queue`].
///
/// Passing `WaitPayload` by value is expensive, so moving `WaitPayload`
/// into and out of `Wait` is done in the outer function `Self::wait` with
/// `#[inline]`.
fn wait_no_queue_inner<System: Kernel>(
    mut lock: CpuLockTokenRefMut<'_, System>,
    wait: &Wait<System>,
) -> Result<(), WaitTimeoutError> {
    let task = wait.task;
    let wait_ref = WaitRef(wait.into());

    debug_assert!(core::ptr::eq(
        wait.task,
        System::state().running_task(lock.borrow_mut()).unwrap()
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
    mut lock: CpuLockTokenRefMut<'_, System>,
    wait: &Wait<System>,
    wait_result: Result<(), WaitTimeoutError>,
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
///
/// `wait_result` must be valid for the wait operation type. For example,
/// if you specify `WaitTimeoutError::Timeout` but the wait operation does not
/// use a timeout, the unblock task will panic immediately (by tripping an error
/// path in [`WaitTimeoutError::expect_not_timeout`]). As a rule of thumb, code
/// outside this module should not pass `WaitTimeoutError::Timeout` to this
/// method.
pub(super) fn interrupt_task<System: Kernel>(
    mut lock: CpuLockTokenRefMut<'_, System>,
    task_cb: &'static TaskCb<System>,
    wait_result: Result<(), WaitTimeoutError>,
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
                let mut accessor = wait_queue_accessor!(&wait_queue.waits, lock.borrow_mut());
                // Safety: `wait_ref` is linked, so it shouldn't return
                //         `ItemError::Unlinked`.
                unsafe { accessor.remove(wait_ref).unwrap_unchecked() };
            }

            // Wake up the task
            complete_wait(lock.borrow_mut(), wait, wait_result);

            Ok(())
        }
        _ => Err(BadObjectStateError::BadObjectState),
    }
}

/// Construct [`timeout::Timeout`] to interrupt the specified task with
/// [`WaitTimeoutError::Timeout`] after a certain period of time.
fn new_timeout_object_for_task<System: Kernel>(
    lock: CpuLockTokenRefMut<'_, System>,
    task_cb: &'static TaskCb<System>,
    duration_time32: timeout::Time32,
) -> timeout::Timeout<System> {
    // Construct a `Timeout`, supplying our callback function
    let param = task_cb as *const _ as usize;
    let timeout_object = timeout::Timeout::new(interrupt_task_by_timeout, param);

    /// The callback function
    fn interrupt_task_by_timeout<System: Kernel>(
        param: usize,
        mut lock: CpuLockGuard<System>,
    ) -> CpuLockGuard<System> {
        // Safety: We are just converting `param` back to the original form
        let task_cb = unsafe { &*(param as *const TaskCb<System>) };

        // Interrupt the task
        match interrupt_task(lock.borrow_mut(), task_cb, Err(WaitTimeoutError::Timeout)) {
            // Even if the task is already unblocked, we don't care
            Ok(()) | Err(BadObjectStateError::BadObjectState) => {}
        }

        lock
    }

    // Configure the `Timeout` to expire in `duration_time32`
    timeout_object.set_expiration_after(lock, duration_time32);

    timeout_object
}
