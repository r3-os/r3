//! Semaphores
use core::{fmt, hash};

use super::{
    raw, raw_cfg, Cfg, DrainSemaphoreError, GetSemaphoreError, PollSemaphoreError, QueueOrder,
    SignalSemaphoreError, WaitSemaphoreError, WaitSemaphoreTimeoutError,
};
use crate::{
    time::Duration,
    utils::{Init, PhantomInvariant},
};

pub use raw::SemaphoreValue;

// ----------------------------------------------------------------------------

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
#[doc = include_str!("../common.md")]
#[repr(transparent)]
pub struct Semaphore<System: raw::KernelSemaphore>(System::SemaphoreId);

impl<System: raw::KernelSemaphore> Clone for Semaphore<System> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<System: raw::KernelSemaphore> Copy for Semaphore<System> {}

impl<System: raw::KernelSemaphore> PartialEq for Semaphore<System> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<System: raw::KernelSemaphore> Eq for Semaphore<System> {}

impl<System: raw::KernelSemaphore> hash::Hash for Semaphore<System> {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        hash::Hash::hash(&self.0, state);
    }
}

impl<System: raw::KernelSemaphore> fmt::Debug for Semaphore<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Semaphore").field(&self.0).finish()
    }
}

impl<System: raw::KernelSemaphore> Semaphore<System> {
    /// Construct a `Semaphore` from `SemaphoreId`.
    ///
    /// # Safety
    ///
    /// The kernel can handle invalid IDs without a problem. However, the
    /// constructed `Semaphore` may point to an object that is not intended to be
    /// manipulated except by its creator. This is usually prevented by making
    /// `Semaphore` an opaque handle, but this safeguard can be circumvented by
    /// this method.
    pub const unsafe fn from_id(id: System::SemaphoreId) -> Self {
        Self(id)
    }

    /// Get the raw `SemaphoreId` value representing this semaphore.
    pub const fn id(self) -> System::SemaphoreId {
        self.0
    }
}

impl<System: raw::KernelSemaphore> Semaphore<System> {
    /// Construct a `SemaphoreDefiner` to define a semaphore in [a
    /// configuration function](crate#static-configuration).
    pub const fn build() -> SemaphoreDefiner<System> {
        SemaphoreDefiner::new()
    }

    /// Remove all permits held by the semaphore.
    pub fn drain(self) -> Result<(), DrainSemaphoreError> {
        // Safety: `Semaphore` represents a permission to access the
        //         referenced object.
        unsafe { System::semaphore_drain(self.0) }
    }

    /// Get the number of permits currently held by the semaphore.
    pub fn get(self) -> Result<SemaphoreValue, GetSemaphoreError> {
        // Safety: `Semaphore` represents a permission to access the
        //         referenced object.
        unsafe { System::semaphore_get(self.0) }
    }

    /// Release `count` permits, returning them to the semaphore.
    pub fn signal(self, count: SemaphoreValue) -> Result<(), SignalSemaphoreError> {
        // Safety: `Semaphore` represents a permission to access the
        //         referenced object.
        unsafe { System::semaphore_signal(self.0, count) }
    }

    /// Release a permit, returning it to the semaphore.
    pub fn signal_one(self) -> Result<(), SignalSemaphoreError> {
        // Safety: `Semaphore` represents a permission to access the
        //         referenced object.
        unsafe { System::semaphore_signal_one(self.0) }
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
    pub fn wait_one(self) -> Result<(), WaitSemaphoreError> {
        // Safety: `Semaphore` represents a permission to access the
        //         referenced object.
        unsafe { System::semaphore_wait_one(self.0) }
    }

    /// [`wait_one`](Self::wait_one) with timeout.
    pub fn wait_one_timeout(self, timeout: Duration) -> Result<(), WaitSemaphoreTimeoutError> {
        // Safety: `Semaphore` represents a permission to access the
        //         referenced object.
        unsafe { System::semaphore_wait_one_timeout(self.0, timeout) }
    }

    /// Non-blocking version of [`wait_one`](Self::wait_one). Returns
    /// immediately with [`PollSemaphoreError::Timeout`] if the unblocking
    /// condition is not satisfied.
    pub fn poll_one(self) -> Result<(), PollSemaphoreError> {
        // Safety: `Semaphore` represents a permission to access the
        //         referenced object.
        unsafe { System::semaphore_poll_one(self.0) }
    }
}

// ----------------------------------------------------------------------------

/// The definer (static builder) for [`Semaphore`].
#[must_use = "must call `finish()` to complete registration"]
pub struct SemaphoreDefiner<System: raw::KernelSemaphore> {
    _phantom: PhantomInvariant<System>,
    initial_value: Option<SemaphoreValue>,
    maximum_value: Option<SemaphoreValue>,
    queue_order: QueueOrder,
}

impl<System: raw::KernelSemaphore> SemaphoreDefiner<System> {
    const fn new() -> Self {
        Self {
            _phantom: Init::INIT,
            initial_value: None,
            maximum_value: None,
            queue_order: QueueOrder::TaskPriority,
        }
    }

    /// \[**Required**\] Specify the initial semaphore value.
    ///
    /// Must be less than or equal to [`maximum`](Self::maximum).
    pub const fn initial(self, initial: SemaphoreValue) -> Self {
        assert!(
            self.initial_value.is_none(),
            "`initial` is already specified"
        );

        Self {
            initial_value: Some(initial),
            ..self
        }
    }

    /// \[**Required**\] Specify the maximum semaphore value.
    pub const fn maximum(self, maximum: SemaphoreValue) -> Self {
        assert!(
            self.maximum_value.is_none(),
            "`maximum` is already specified"
        );

        Self {
            maximum_value: Some(maximum),
            ..self
        }
    }

    /// Specify how tasks are sorted in the wait queue of the semaphore.
    /// Defaults to [`QueueOrder::TaskPriority`] when unspecified.
    pub const fn queue_order(self, queue_order: QueueOrder) -> Self {
        Self {
            queue_order,
            ..self
        }
    }

    /// Complete the definition of a semaphore, returning a reference to the
    /// semaphore.
    pub const fn finish<C: ~const raw_cfg::CfgSemaphore<System = System>>(
        self,
        c: &mut Cfg<C>,
    ) -> Semaphore<System> {
        let initial_value = if let Some(x) = self.initial_value {
            x
        } else {
            panic!("`initial` is not specified")
        };
        let maximum_value = if let Some(x) = self.maximum_value {
            x
        } else {
            panic!("`maximum` is not specified")
        };

        let id = c.raw().semaphore_define(
            raw_cfg::SemaphoreDescriptor {
                phantom: Init::INIT,
                initial: initial_value,
                maximum: maximum_value,
                queue_order: self.queue_order,
            },
            (),
        );
        unsafe { Semaphore::from_id(id) }
    }
}
