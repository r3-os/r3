//! Semaphores
use core::{fmt, hash, marker::PhantomData};

use super::{
    state, task, timeout, utils,
    wait::{WaitPayload, WaitQueue},
    BadIdError, DrainSemaphoreError, GetSemaphoreError, Id, Kernel, PollSemaphoreError, Port,
    SignalSemaphoreError, WaitSemaphoreError, WaitSemaphoreTimeoutError,
};
use crate::{time::Duration, utils::Init};

/// Unsigned integer type representing the number of permits held by a
/// [semaphore].
///
/// [semaphore]: Semaphore
///
/// <div class="admonition-follows"></div>
///
/// > **Rationale:** On the one hand, using a data type with a target-dependent
/// > size can hurt portability. On the other hand, a fixed-size data type such
/// > as `u32` can significantly increase the runtime overhead on extremely
/// > constrained targets such as AVR and MSP430. In addition, many RISC targets
/// > handle small data types less efficiently. The portability issue shouldn't
/// > pose a problem in practice.
#[doc(include = "../common.md")]
pub type SemaphoreValue = usize;

/// Represents a single semaphore in a system.
///
/// A semaphore maintains a set of permits that can be acquired (possibly
/// blocking) or released by application code. The number of permits held by a
/// semaphore is called the semaphore's *value* and represented by
/// [`SemaphoreValue`].
///
/// This type is ABI-compatible with [`Id`].
///
/// <div class="admonition-follows"></div>
///
/// > **Relation to Other Specifications:** Present in almost every real-time
/// > operating system.
#[doc(include = "../common.md")]
#[repr(transparent)]
pub struct Semaphore<System>(Id, PhantomData<System>);

impl<System> Clone for Semaphore<System> {
    fn clone(&self) -> Self {
        Self(self.0, self.1)
    }
}

impl<System> Copy for Semaphore<System> {}

impl<System> PartialEq for Semaphore<System> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<System> Eq for Semaphore<System> {}

impl<System> hash::Hash for Semaphore<System> {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        hash::Hash::hash(&self.0, state);
    }
}

impl<System> fmt::Debug for Semaphore<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Semaphore").field(&self.0).finish()
    }
}

impl<System> Semaphore<System> {
    /// Construct a `Semaphore` from `Id`.
    ///
    /// # Safety
    ///
    /// The kernel can handle invalid IDs without a problem. However, the
    /// constructed `Semaphore` may point to an object that is not intended to be
    /// manipulated except by its creator. This is usually prevented by making
    /// `Semaphore` an opaque handle, but this safeguard can be circumvented by
    /// this method.
    pub const unsafe fn from_id(id: Id) -> Self {
        Self(id, PhantomData)
    }

    /// Get the raw `Id` value representing this semaphore.
    pub const fn id(self) -> Id {
        self.0
    }
}

impl<System: Kernel> Semaphore<System> {
    fn semaphore_cb(self) -> Result<&'static SemaphoreCb<System>, BadIdError> {
        System::get_semaphore_cb(self.0.get() - 1).ok_or(BadIdError::BadId)
    }

    /// Remove all permits held by the semaphore.
    #[cfg_attr(not(feature = "inline-syscall"), inline(never))]
    pub fn drain(self) -> Result<(), DrainSemaphoreError> {
        let mut lock = utils::lock_cpu::<System>()?;
        let semaphore_cb = self.semaphore_cb()?;
        semaphore_cb.value.replace(&mut *lock, 0);
        Ok(())
    }

    /// Get the number of permits currently held by the semaphore.
    #[cfg_attr(not(feature = "inline-syscall"), inline(never))]
    pub fn get(self) -> Result<SemaphoreValue, GetSemaphoreError> {
        let lock = utils::lock_cpu::<System>()?;
        let semaphore_cb = self.semaphore_cb()?;
        Ok(semaphore_cb.value.get(&*lock))
    }

    /// Release `count` permits, returning them to the semaphore.
    #[cfg_attr(not(feature = "inline-syscall"), inline(never))]
    pub fn signal(self, count: SemaphoreValue) -> Result<(), SignalSemaphoreError> {
        let lock = utils::lock_cpu::<System>()?;
        let semaphore_cb = self.semaphore_cb()?;
        signal(semaphore_cb, lock, count)
    }

    /// Release a permit, returning it to the semaphore.
    pub fn signal_one(self) -> Result<(), SignalSemaphoreError> {
        self.signal(1)
    }

    /// Acquire a permit, potentially blocking the calling thread until one is
    /// available.
    ///
    /// This system service may block. Therefore, calling this method is not
    /// allowed in [a non-waitable context] and will return `Err(BadContext)`.
    ///
    /// [a non-waitable context]: crate#contexts
    ///
    /// <div class="admonition-follows"></div>
    ///
    /// > **Rationale:** Multi-wait (waiting for more than one permit) is not
    /// > supported because it introduces additional complexity to the wait
    /// > queue mechanism by requiring it to reevaluate wait conditions after
    /// > reordering the queue.
    /// >
    /// > The support for multi-wait is relatively rare among operating systems.
    /// > It's not supported by POSIX, RTEMS, TOPPERS, VxWorks, nor Win32. The
    /// > rare exception is μT-Kernel.
    #[cfg_attr(not(feature = "inline-syscall"), inline(never))]
    pub fn wait_one(self) -> Result<(), WaitSemaphoreError> {
        let lock = utils::lock_cpu::<System>()?;
        state::expect_waitable_context::<System>()?;
        let semaphore_cb = self.semaphore_cb()?;

        wait_one(semaphore_cb, lock)
    }

    /// [`wait_one`](Self::wait_one) with timeout.
    #[cfg_attr(not(feature = "inline-syscall"), inline(never))]
    pub fn wait_one_timeout(self, timeout: Duration) -> Result<(), WaitSemaphoreTimeoutError> {
        let time32 = timeout::time32_from_duration(timeout)?;
        let lock = utils::lock_cpu::<System>()?;
        state::expect_waitable_context::<System>()?;
        let semaphore_cb = self.semaphore_cb()?;

        wait_one_timeout(semaphore_cb, lock, time32)
    }

    /// Non-blocking version of [`wait_one`](Self::wait_one). Returns
    /// immediately with [`PollSemaphoreError::Timeout`] if the unblocking
    /// condition is not satisfied.
    #[cfg_attr(not(feature = "inline-syscall"), inline(never))]
    pub fn poll_one(self) -> Result<(), PollSemaphoreError> {
        let lock = utils::lock_cpu::<System>()?;
        let semaphore_cb = self.semaphore_cb()?;

        poll_one(semaphore_cb, lock)
    }
}

/// *Semaphore control block* - the state data of an event group.
#[doc(hidden)]
pub struct SemaphoreCb<System: Port> {
    pub(super) value: utils::CpuLockCell<System, SemaphoreValue>,
    pub(super) max_value: SemaphoreValue,

    pub(super) wait_queue: WaitQueue<System>,
}

impl<System: Port> Init for SemaphoreCb<System> {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self {
        value: Init::INIT,
        max_value: Init::INIT,
        wait_queue: Init::INIT,
    };
}

impl<System: Kernel> fmt::Debug for SemaphoreCb<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SemaphoreCb")
            .field("self", &(self as *const _))
            .field("value", &self.value)
            .field("max_value", &self.max_value)
            .field("wait_queue", &self.wait_queue)
            .finish()
    }
}

fn poll_one<System: Kernel>(
    semaphore_cb: &'static SemaphoreCb<System>,
    mut lock: utils::CpuLockGuard<System>,
) -> Result<(), PollSemaphoreError> {
    if poll_core(semaphore_cb.value.write(&mut *lock)) {
        Ok(())
    } else {
        Err(PollSemaphoreError::Timeout)
    }
}

fn wait_one<System: Kernel>(
    semaphore_cb: &'static SemaphoreCb<System>,
    mut lock: utils::CpuLockGuard<System>,
) -> Result<(), WaitSemaphoreError> {
    if poll_core(semaphore_cb.value.write(&mut *lock)) {
        Ok(())
    } else {
        // The current state does not satify the wait condition. In this case,
        // start waiting. The wake-upper is responsible for using `poll_core`
        // to complete the effect of the wait operation.
        semaphore_cb
            .wait_queue
            .wait(lock.borrow_mut(), WaitPayload::Semaphore)?;

        Ok(())
    }
}

fn wait_one_timeout<System: Kernel>(
    semaphore_cb: &'static SemaphoreCb<System>,
    mut lock: utils::CpuLockGuard<System>,
    time32: timeout::Time32,
) -> Result<(), WaitSemaphoreTimeoutError> {
    if poll_core(semaphore_cb.value.write(&mut *lock)) {
        Ok(())
    } else {
        // The current state does not satify the wait condition. In this case,
        // start waiting. The wake-upper is responsible for using `poll_core`
        // to complete the effect of the wait operation.
        semaphore_cb
            .wait_queue
            .wait_timeout(lock.borrow_mut(), WaitPayload::Semaphore, time32)?;

        Ok(())
    }
}

/// Check if the current state of a semaphore, `value`, satisfies the wait
/// condition.
///
/// If `value` satisfies the wait condition, this function updates `value` and
/// returns `true`. Otherwise, it returns `false`.
#[inline]
fn poll_core(value: &mut SemaphoreValue) -> bool {
    if *value > 0 {
        *value -= 1;
        true
    } else {
        false
    }
}

fn signal<System: Kernel>(
    semaphore_cb: &'static SemaphoreCb<System>,
    mut lock: utils::CpuLockGuard<System>,
    mut count: SemaphoreValue,
) -> Result<(), SignalSemaphoreError> {
    let value = semaphore_cb.value.get(&*lock);

    if semaphore_cb.max_value - value < count {
        return Err(SignalSemaphoreError::QueueOverflow);
    }

    let orig_count = count;

    // This is equivalent to using `wake_up_all_conditional` and calling
    // `poll_core` for each waiting task, but is (presumably) more efficient
    while count > 0 {
        if semaphore_cb.wait_queue.wake_up_one(lock.borrow_mut()) {
            // We just woke up a task. Give one permit to that task.
            count -= 1;
        } else {
            // There's no more task to wake up; deposit the remaining permits to
            // the semaphore
            semaphore_cb.value.replace(&mut *lock, value + count);
            break;
        }
    }

    // If we woke up at least one task in the process, call
    // `unlock_cpu_and_check_preemption`
    if count != orig_count {
        task::unlock_cpu_and_check_preemption(lock);
    }

    Ok(())
}
