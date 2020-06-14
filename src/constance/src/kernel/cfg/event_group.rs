use core::{marker::PhantomData, num::NonZeroUsize};

use crate::kernel::{cfg::CfgBuilder, event_group, utils::CpuLockCell, wait, Port};

/// Used by `new_event_group!` in configuraton functions
#[doc(hidden)]
pub struct CfgEventGroupBuilder<System> {
    _phantom: PhantomData<System>,
    initial_bits: event_group::EventGroupBits,
    queue_order: wait::QueueOrder,
}

impl<System: Port> CfgEventGroupBuilder<System> {
    pub const fn new() -> Self {
        Self {
            _phantom: PhantomData,
            initial_bits: 0,
            queue_order: wait::QueueOrder::TaskPriority,
        }
    }

    pub const fn initial(self, initial: event_group::EventGroupBits) -> Self {
        Self {
            initial_bits: initial,
            ..self
        }
    }

    pub const fn queue_order(self, queue_order: wait::QueueOrder) -> Self {
        Self {
            queue_order,
            ..self
        }
    }

    pub const fn finish(self, cfg: &mut CfgBuilder<System>) -> event_group::EventGroup<System> {
        cfg.event_groups.push(CfgBuilderEventGroup {
            initial_bits: self.initial_bits,
            queue_order: self.queue_order,
        });

        unsafe {
            event_group::EventGroup::from_id(NonZeroUsize::new_unchecked(cfg.event_groups.len()))
        }
    }
}

#[doc(hidden)]
pub struct CfgBuilderEventGroup {
    initial_bits: event_group::EventGroupBits,
    queue_order: wait::QueueOrder,
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
    pub const fn to_state<System: Port>(&self) -> event_group::EventGroupCb<System> {
        event_group::EventGroupCb {
            bits: CpuLockCell::new(self.initial_bits),
            wait_queue: wait::WaitQueue::new(self.queue_order),
        }
    }
}
