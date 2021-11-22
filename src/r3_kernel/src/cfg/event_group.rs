use core::num::NonZeroUsize;
use r3::kernel::{
    raw_cfg::{CfgEventGroup, EventGroupDescriptor},
    EventGroupBits, QueueOrder,
};

use crate::{cfg::CfgBuilder, event_group, klock::CpuLockCell, wait, KernelTraits, Port};

unsafe impl<Traits: KernelTraits> const CfgEventGroup for CfgBuilder<Traits> {
    fn event_group_define(
        &mut self,
        EventGroupDescriptor {
            phantom: _,
            initial_bits,
            queue_order,
        }: EventGroupDescriptor<Self::System>,
        _properties: impl r3::bag::Bag,
    ) -> event_group::EventGroupId {
        let inner = &mut self.inner;

        inner.event_groups.push(CfgBuilderEventGroup {
            initial_bits,
            queue_order,
        });

        unsafe { NonZeroUsize::new_unchecked(inner.event_groups.len()) }
    }
}

#[doc(hidden)]
pub struct CfgBuilderEventGroup {
    initial_bits: EventGroupBits,
    queue_order: QueueOrder,
}

impl Clone for CfgBuilderEventGroup {
    fn clone(&self) -> Self {
        Self {
            initial_bits: self.initial_bits,
            queue_order: self.queue_order,
        }
    }
}

impl Copy for CfgBuilderEventGroup {}

impl CfgBuilderEventGroup {
    pub const fn to_state<Traits: Port>(&self) -> event_group::EventGroupCb<Traits> {
        event_group::EventGroupCb {
            bits: CpuLockCell::new(self.initial_bits),
            wait_queue: wait::WaitQueue::new(self.queue_order),
        }
    }
}
