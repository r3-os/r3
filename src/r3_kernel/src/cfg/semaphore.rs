use core::{marker::PhantomData, num::NonZeroUsize};

use crate::kernel::{cfg::CfgBuilder, semaphore, utils::CpuLockCell, wait, Port};

impl<System: Port> semaphore::Semaphore<System> {
    /// Construct a `CfgTaskBuilder` to define a semaphore in [a configuration
    /// function](crate#static-configuration).
    pub const fn build() -> CfgSemaphoreBuilder<System> {
        CfgSemaphoreBuilder::new()
    }
}

/// Configuration builder type for [`Semaphore`].
///
/// [`Semaphore`]: crate::kernel::Semaphore
#[must_use = "must call `finish()` to complete registration"]
pub struct CfgSemaphoreBuilder<System> {
    _phantom: PhantomData<System>,
    initial_value: Option<semaphore::SemaphoreValue>,
    maximum_value: Option<semaphore::SemaphoreValue>,
    queue_order: wait::QueueOrder,
}

impl<System: Port> CfgSemaphoreBuilder<System> {
    const fn new() -> Self {
        Self {
            _phantom: PhantomData,
            initial_value: None,
            maximum_value: None,
            queue_order: wait::QueueOrder::TaskPriority,
        }
    }

    /// \[**Required**\] Specify the initial semaphore value.
    ///
    /// Must be less than or equal to [`maximum`](Self::maximum).
    pub const fn initial(self, initial: semaphore::SemaphoreValue) -> Self {
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
    pub const fn maximum(self, maximum: semaphore::SemaphoreValue) -> Self {
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
    ///
    /// [`QueueOrder::TaskPriority`]: wait::QueueOrder::TaskPriority
    pub const fn queue_order(self, queue_order: wait::QueueOrder) -> Self {
        Self {
            queue_order,
            ..self
        }
    }

    /// Complete the definition of a semaphore, returning a reference to the
    /// semaphore.
    pub const fn finish(self, cfg: &mut CfgBuilder<System>) -> semaphore::Semaphore<System> {
        let inner = &mut cfg.inner;

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

        assert!(
            initial_value <= maximum_value,
            "`initial` must be less than or equal to `maximum`"
        );

        inner.semaphores.push(CfgBuilderSemaphore {
            initial_value,
            maximum_value,
            queue_order: self.queue_order,
        });

        unsafe {
            semaphore::Semaphore::from_id(NonZeroUsize::new_unchecked(inner.semaphores.len()))
        }
    }
}

#[doc(hidden)]
pub struct CfgBuilderSemaphore {
    initial_value: semaphore::SemaphoreValue,
    maximum_value: semaphore::SemaphoreValue,
    queue_order: wait::QueueOrder,
}

impl Clone for CfgBuilderSemaphore {
    fn clone(&self) -> Self {
        Self {
            initial_value: self.initial_value,
            maximum_value: self.maximum_value,
            queue_order: self.queue_order,
        }
    }
}

impl Copy for CfgBuilderSemaphore {}

impl CfgBuilderSemaphore {
    pub const fn to_state<System: Port>(&self) -> semaphore::SemaphoreCb<System> {
        semaphore::SemaphoreCb {
            value: CpuLockCell::new(self.initial_value),
            max_value: self.maximum_value,
            wait_queue: wait::WaitQueue::new(self.queue_order),
        }
    }
}
