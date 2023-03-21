//! Semaphores
use core::fmt;
use r3_core::{
    kernel::{
        DrainSemaphoreError, GetSemaphoreError, PollSemaphoreError, SemaphoreValue,
        SignalSemaphoreError, WaitSemaphoreError, WaitSemaphoreTimeoutError,
    },
    time::Duration,
    utils::Init,
};

use crate::{
    error::NoAccessError,
    klock, state, task, timeout,
    wait::{WaitPayload, WaitQueue},
    Id, KernelTraits, Port, System,
};

pub(super) type SemaphoreId = Id;

impl<Traits: KernelTraits> System<Traits> {
    /// Get the [`SemaphoreCb`] for the specified raw ID.
    ///
    /// # Safety
    ///
    /// See [`crate::bad_id`].
    #[inline]
    unsafe fn semaphore_cb(
        this: SemaphoreId,
    ) -> Result<&'static SemaphoreCb<Traits>, NoAccessError> {
        Traits::get_semaphore_cb(this.get() - 1).ok_or_else(|| unsafe { crate::bad_id() })
    }
}

unsafe impl<Traits: KernelTraits> r3_core::kernel::raw::KernelSemaphore for System<Traits> {
    type RawSemaphoreId = SemaphoreId;

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_semaphore_drain(this: SemaphoreId) -> Result<(), DrainSemaphoreError> {
        let mut lock = klock::lock_cpu::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let semaphore_cb = unsafe { Self::semaphore_cb(this)? };
        semaphore_cb.value.replace(&mut *lock, 0);
        Ok(())
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_semaphore_get(this: SemaphoreId) -> Result<SemaphoreValue, GetSemaphoreError> {
        let lock = klock::lock_cpu::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let semaphore_cb = unsafe { Self::semaphore_cb(this)? };
        Ok(semaphore_cb.value.get(&*lock))
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_semaphore_signal(
        this: SemaphoreId,
        count: SemaphoreValue,
    ) -> Result<(), SignalSemaphoreError> {
        let lock = klock::lock_cpu::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let semaphore_cb = unsafe { Self::semaphore_cb(this)? };
        signal(semaphore_cb, lock, count)
    }

    unsafe fn raw_semaphore_signal_one(this: SemaphoreId) -> Result<(), SignalSemaphoreError> {
        unsafe { Self::raw_semaphore_signal(this, 1) }
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_semaphore_wait_one(this: SemaphoreId) -> Result<(), WaitSemaphoreError> {
        let lock = klock::lock_cpu::<Traits>()?;
        state::expect_waitable_context::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let semaphore_cb = unsafe { Self::semaphore_cb(this)? };

        wait_one(semaphore_cb, lock)
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_semaphore_wait_one_timeout(
        this: SemaphoreId,
        timeout: Duration,
    ) -> Result<(), WaitSemaphoreTimeoutError> {
        let time32 = timeout::time32_from_duration(timeout)?;
        let lock = klock::lock_cpu::<Traits>()?;
        state::expect_waitable_context::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let semaphore_cb = unsafe { Self::semaphore_cb(this)? };

        wait_one_timeout(semaphore_cb, lock, time32)
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_semaphore_poll_one(this: SemaphoreId) -> Result<(), PollSemaphoreError> {
        let lock = klock::lock_cpu::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let semaphore_cb = unsafe { Self::semaphore_cb(this)? };

        poll_one(semaphore_cb, lock)
    }
}

/// *Semaphore control block* - the state data of an event group.
#[doc(hidden)]
pub struct SemaphoreCb<Traits: Port> {
    pub(super) value: klock::CpuLockCell<Traits, SemaphoreValue>,
    pub(super) max_value: SemaphoreValue,

    pub(super) wait_queue: WaitQueue<Traits>,
}

impl<Traits: Port> Init for SemaphoreCb<Traits> {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self {
        value: Init::INIT,
        max_value: Init::INIT,
        wait_queue: Init::INIT,
    };
}

impl<Traits: KernelTraits> fmt::Debug for SemaphoreCb<Traits> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SemaphoreCb")
            .field("self", &(self as *const _))
            .field("value", &self.value)
            .field("max_value", &self.max_value)
            .field("wait_queue", &self.wait_queue)
            .finish()
    }
}

#[inline]
fn poll_one<Traits: KernelTraits>(
    semaphore_cb: &'static SemaphoreCb<Traits>,
    mut lock: klock::CpuLockGuard<Traits>,
) -> Result<(), PollSemaphoreError> {
    if poll_core(semaphore_cb.value.write(&mut *lock)) {
        Ok(())
    } else {
        Err(PollSemaphoreError::Timeout)
    }
}

#[inline]
fn wait_one<Traits: KernelTraits>(
    semaphore_cb: &'static SemaphoreCb<Traits>,
    mut lock: klock::CpuLockGuard<Traits>,
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

#[inline]
fn wait_one_timeout<Traits: KernelTraits>(
    semaphore_cb: &'static SemaphoreCb<Traits>,
    mut lock: klock::CpuLockGuard<Traits>,
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

#[inline]
fn signal<Traits: KernelTraits>(
    semaphore_cb: &'static SemaphoreCb<Traits>,
    mut lock: klock::CpuLockGuard<Traits>,
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
