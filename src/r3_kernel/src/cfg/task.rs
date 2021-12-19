use core::num::NonZeroUsize;
use r3::{
    kernel::{
        raw_cfg::{CfgTask, TaskDescriptor},
        task::StackHunk,
    },
    utils::ConstDefault,
};

use crate::{cfg::CfgBuilder, klock::CpuLockCell, task, KernelTraits};

unsafe impl<Traits: KernelTraits> const CfgTask for CfgBuilder<Traits> {
    fn task_define<Properties: ~const r3::bag::Bag>(
        &mut self,
        TaskDescriptor {
            phantom: _,
            start,
            param,
            active,
            priority,
            stack_size,
        }: TaskDescriptor<Self::System>,
        properties: Properties,
    ) -> task::TaskId {
        // FIXME: `Option::unwrap_or` isn't `const fn` yet
        let mut stack = task::StackHunk::auto(if let Some(x) = stack_size {
            x
        } else {
            Traits::STACK_DEFAULT_SIZE
        });

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

        let inner = &mut self.inner;

        inner.tasks.push(CfgBuilderTask {
            start,
            param,
            stack,
            priority,
            active,
        });

        unsafe { NonZeroUsize::new_unchecked(inner.tasks.len()) }
    }
}

#[doc(hidden)]
pub struct CfgBuilderTask<Traits: KernelTraits> {
    start: fn(usize),
    param: usize,
    pub(super) stack: task::StackHunk<Traits>,
    priority: usize,
    active: bool,
}

impl<Traits: KernelTraits> Clone for CfgBuilderTask<Traits> {
    fn clone(&self) -> Self {
        Self {
            start: self.start,
            param: self.param,
            stack: self.stack,
            priority: self.priority,
            active: self.active,
        }
    }
}

impl<Traits: KernelTraits> Copy for CfgBuilderTask<Traits> {}

impl<Traits: KernelTraits> CfgBuilderTask<Traits> {
    pub const fn to_state(&self, attr: &'static task::TaskAttr<Traits>) -> task::TaskCb<Traits> {
        // `self.priority` has already been checked by `to_attr`
        let priority = Traits::TASK_PRIORITY_LEVELS[self.priority];

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
            ready_queue_data: ConstDefault::DEFAULT,
            wait: ConstDefault::DEFAULT,
            park_token: CpuLockCell::new(false),
            last_mutex_held: CpuLockCell::new(None),
        }
    }

    pub const fn to_attr(&self) -> task::TaskAttr<Traits> {
        task::TaskAttr {
            entry_point: self.start,
            entry_param: self.param,
            stack: self.stack,
            priority: if self.priority < Traits::NUM_TASK_PRIORITY_LEVELS {
                Traits::TASK_PRIORITY_LEVELS[self.priority]
            } else {
                panic!("task's `priority` must be less than `num_task_priority_levels`");
            },
        }
    }
}
