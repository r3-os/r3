use core::num::NonZeroUsize;
use r3_core::{
    closure::Closure,
    kernel::{
        raw_cfg::{CfgTask, TaskDescriptor},
        task::StackHunk,
    },
    utils::Init,
};

use crate::{cfg::CfgBuilder, klock::CpuLockCell, task, KernelCfg1, KernelTraits};

unsafe impl<Traits: KernelTraits> const CfgTask for CfgBuilder<Traits> {
    fn task_define<Properties: ~const r3_core::bag::Bag>(
        &mut self,
        TaskDescriptor {
            phantom: _,
            start,
            active,
            priority,
            stack_size,
        }: TaskDescriptor<Self::System>,
        properties: Properties,
    ) -> task::TaskId {
        let mut stack = task::StackHunk::auto(stack_size.unwrap_or(Traits::STACK_DEFAULT_SIZE));

        if let Some(hunk) = properties.get::<StackHunk<Self::System>>() {
            let stack_size = if let Some(stack_size) = stack_size {
                stack_size
            } else {
                panic!(
                    "if a task stack is specified by `StackHunk`, the stack size must \
                    be specified explicitly"
                )
            };
            stack = task::StackHunk::from_hunk(hunk.hunk(), stack_size);
        }

        self.tasks.push(CfgBuilderTask {
            start,
            stack,
            priority,
            active,
        });

        unsafe { NonZeroUsize::new_unchecked(self.tasks.len()) }
    }
}

#[doc(hidden)]
pub struct CfgBuilderTask<Traits: KernelTraits> {
    start: Closure,
    pub(super) stack: task::StackHunk<Traits>,
    priority: usize,
    active: bool,
}

impl<Traits: KernelTraits> Clone for CfgBuilderTask<Traits> {
    fn clone(&self) -> Self {
        Self {
            start: self.start,
            stack: self.stack,
            priority: self.priority,
            active: self.active,
        }
    }
}

impl<Traits: KernelTraits> Copy for CfgBuilderTask<Traits> {}

impl<Traits: KernelTraits> CfgBuilderTask<Traits> {
    pub const fn to_state(&self, attr: &'static task::TaskAttr<Traits>) -> task::TaskCb<Traits>
    where
        Traits: ~const KernelCfg1,
    {
        // `self.priority` has already been checked by `to_attr`
        let priority = Traits::to_task_priority(self.priority).unwrap();

        task::TaskCb {
            port_task_state: Traits::PORT_TASK_STATE_INIT,
            attr,
            base_priority: CpuLockCell::new(priority),
            effective_priority: CpuLockCell::new(priority),
            st: CpuLockCell::new(if self.active {
                task::TaskSt::PendingActivation
            } else {
                task::TaskSt::Dormant
            }),
            ready_queue_data: Init::INIT,
            wait: Init::INIT,
            park_token: CpuLockCell::new(false),
            last_mutex_held: CpuLockCell::new(None),
        }
    }

    pub const fn to_attr(&self) -> task::TaskAttr<Traits>
    where
        Traits: ~const KernelCfg1,
    {
        let (entry_point, entry_param) = self.start.as_raw_parts();
        task::TaskAttr {
            entry_point,
            entry_param,
            stack: self.stack,
            priority: Traits::to_task_priority(self.priority)
                .expect("task's `priority` must be less than `num_task_priority_levels`"),
        }
    }
}
