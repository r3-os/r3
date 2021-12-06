use core::num::NonZeroUsize;
use r3::kernel::{
    raw_cfg::{CfgSemaphore, SemaphoreDescriptor},
    SemaphoreValue,
};

use crate::{cfg::CfgBuilder, klock::CpuLockCell, semaphore, wait, KernelTraits, Port};

unsafe impl<Traits: KernelTraits> const CfgSemaphore for CfgBuilder<Traits> {
    fn semaphore_define<Properties: ~const r3::bag::Bag>(
        &mut self,
        SemaphoreDescriptor {
            phantom: _,
            initial,
            maximum,
            queue_order,
        }: SemaphoreDescriptor<Self::System>,
        _properties: Properties,
    ) -> semaphore::SemaphoreId {
        let inner = &mut self.inner;

        assert!(
            initial <= maximum,
            "`initial` must be less than or equal to `maximum`"
        );

        inner.semaphores.push(CfgBuilderSemaphore {
            initial,
            maximum,
            queue_order: wait::QueueOrder::from(queue_order),
        });

        unsafe { NonZeroUsize::new_unchecked(inner.semaphores.len()) }
    }
}

#[doc(hidden)]
pub struct CfgBuilderSemaphore {
    initial: SemaphoreValue,
    maximum: SemaphoreValue,
    queue_order: wait::QueueOrder,
}

impl Clone for CfgBuilderSemaphore {
    fn clone(&self) -> Self {
        Self {
            initial: self.initial,
            maximum: self.maximum,
            queue_order: self.queue_order,
        }
    }
}

impl Copy for CfgBuilderSemaphore {}

impl CfgBuilderSemaphore {
    pub const fn to_state<System: Port>(&self) -> semaphore::SemaphoreCb<System> {
        semaphore::SemaphoreCb {
            value: CpuLockCell::new(self.initial),
            max_value: self.maximum,
            wait_queue: wait::WaitQueue::new(self.queue_order),
        }
    }
}
