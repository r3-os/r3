use core::num::NonZeroUsize;
use r3_core::kernel::{
    raw_cfg::{CfgMutex, MutexDescriptor},
    MutexProtocol,
};

use crate::{cfg::CfgBuilder, klock::CpuLockCell, mutex, wait, KernelCfg1, KernelTraits, Port};

unsafe impl<Traits: KernelTraits> const CfgMutex for CfgBuilder<Traits> {
    fn mutex_define<Properties: ~const r3_core::bag::Bag>(
        &mut self,
        MutexDescriptor {
            phantom: _,
            protocol,
        }: MutexDescriptor<Self::System>,
        _properties: Properties,
    ) -> mutex::MutexId {
        self.mutexes.push(CfgBuilderMutex { protocol });

        unsafe { NonZeroUsize::new_unchecked(self.mutexes.len()) }
    }
}

#[doc(hidden)]
#[derive(Copy, Clone)]
pub struct CfgBuilderMutex {
    protocol: MutexProtocol,
}

impl CfgBuilderMutex {
    pub const fn to_state<Traits: Port + ~const KernelCfg1>(&self) -> mutex::MutexCb<Traits> {
        mutex::MutexCb {
            ceiling: match self.protocol {
                MutexProtocol::None => None,
                MutexProtocol::Ceiling(ceiling) => Some(Traits::to_task_priority(ceiling).expect(
                    "mutex's priority ceiling must be less than `num_task_priority_levels`",
                )),

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
