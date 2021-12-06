use core::num::NonZeroUsize;
use r3::kernel::{
    raw_cfg::{CfgMutex, MutexDescriptor},
    MutexProtocol,
};

use crate::{cfg::CfgBuilder, klock::CpuLockCell, mutex, wait, KernelTraits, Port};

unsafe impl<Traits: KernelTraits> const CfgMutex for CfgBuilder<Traits> {
    fn mutex_define<Properties: ~const r3::bag::Bag>(
        &mut self,
        MutexDescriptor {
            phantom: _,
            protocol,
        }: MutexDescriptor<Self::System>,
        _properties: Properties,
    ) -> mutex::MutexId {
        let inner = &mut self.inner;

        inner.mutexes.push(CfgBuilderMutex { protocol });

        unsafe { NonZeroUsize::new_unchecked(inner.mutexes.len()) }
    }
}

#[doc(hidden)]
#[derive(Copy, Clone)]
pub struct CfgBuilderMutex {
    protocol: MutexProtocol,
}

impl CfgBuilderMutex {
    pub const fn to_state<System: Port>(&self) -> mutex::MutexCb<System> {
        mutex::MutexCb {
            ceiling: match self.protocol {
                MutexProtocol::None => None,
                MutexProtocol::Ceiling(ceiling) => {
                    if ceiling < System::NUM_TASK_PRIORITY_LEVELS {
                        Some(System::TASK_PRIORITY_LEVELS[ceiling])
                    } else {
                        panic!(
                            "mutex's priority ceiling must be less than `num_task_priority_levels`"
                        );
                    }
                }

                // The default value is implementation-defined
                _ => None,
            },
            inconsistent: CpuLockCell::new(false),
            wait_queue: wait::WaitQueue::new(wait::QueueOrder::TaskPriority),
            prev_mutex_held: CpuLockCell::new(None),
            owning_task: CpuLockCell::new(None),
        }
    }
}
