use core::{marker::PhantomData, num::NonZeroUsize};

use crate::kernel::{cfg::CfgBuilder, mutex, utils::CpuLockCell, wait, Port};

impl<System: Port> mutex::Mutex<System> {
    /// Construct a `CfgTaskBuilder` to define a mutex in [a configuration
    /// function](crate#static-configuration).
    pub const fn build() -> CfgMutexBuilder<System> {
        CfgMutexBuilder::new()
    }
}

/// Configuration builder type for [`Mutex`].
///
/// [`Mutex`]: crate::kernel::Mutex
#[must_use = "must call `finish()` to complete registration"]
pub struct CfgMutexBuilder<System> {
    protocol: mutex::MutexProtocol,
    _phantom: PhantomData<System>,
}

impl<System: Port> CfgMutexBuilder<System> {
    const fn new() -> Self {
        Self {
            protocol: mutex::MutexProtocol::None,
            _phantom: PhantomData,
        }
    }

    /// Specify the mutex's protocol. Defaults to `None` when unspecified.
    pub const fn protocol(self, protocol: mutex::MutexProtocol) -> Self {
        Self { protocol, ..self }
    }

    /// Complete the definition of a mutex, returning a reference to the
    /// mutex.
    pub const fn finish(self, cfg: &mut CfgBuilder<System>) -> mutex::Mutex<System> {
        let inner = &mut cfg.inner;

        inner.mutexes.push(CfgBuilderMutex {
            protocol: self.protocol,
        });

        unsafe { mutex::Mutex::from_id(NonZeroUsize::new_unchecked(inner.mutexes.len())) }
    }
}

#[doc(hidden)]
#[derive(Copy, Clone)]
pub struct CfgBuilderMutex {
    protocol: mutex::MutexProtocol,
}

impl CfgBuilderMutex {
    pub const fn to_state<System: Port>(&self) -> mutex::MutexCb<System> {
        mutex::MutexCb {
            ceiling: match self.protocol {
                mutex::MutexProtocol::None => None,
                mutex::MutexProtocol::Ceiling(ceiling) => {
                    Some(System::TASK_PRIORITY_LEVELS[ceiling])
                }
            },
            inconsistent: CpuLockCell::new(false),
            wait_queue: wait::WaitQueue::new(wait::QueueOrder::TaskPriority),
            prev_mutex_held: CpuLockCell::new(None),
            owning_task: CpuLockCell::new(None),
        }
    }
}
