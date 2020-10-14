use core::{marker::PhantomData, num::NonZeroUsize};

use crate::{
    kernel::{cfg::CfgBuilder, hunk, task, utils::CpuLockCell, Port},
    utils::Init,
};

impl<System: Port> task::Task<System> {
    /// Construct a `CfgTaskBuilder` to define a task in [a configuration
    /// function](crate#static-configuration).
    pub const fn build() -> CfgTaskBuilder<System> {
        CfgTaskBuilder::new()
    }
}

/// Configuration builder type for [`Task`].
///
/// [`Task`]: crate::kernel::Task
#[must_use = "must call `finish()` to complete registration"]
pub struct CfgTaskBuilder<System> {
    _phantom: PhantomData<System>,
    start: Option<fn(usize)>,
    param: usize,
    stack: Option<TaskStack<System>>,
    priority: Option<usize>,
    active: bool,
}

enum TaskStack<System> {
    Auto(usize),
    Hunk(task::StackHunk<System>),
    // TODO: Externally supplied stack? It's blocked by
    //       <https://github.com/rust-lang/const-eval/issues/11>, I think
}

impl<System: Port> CfgTaskBuilder<System> {
    const fn new() -> Self {
        Self {
            _phantom: PhantomData,
            start: None,
            param: 0,
            stack: None,
            priority: None,
            active: false,
        }
    }

    /// [**Required**] Specify the task's entry point.
    pub const fn start(self, start: fn(usize)) -> Self {
        Self {
            start: Some(start),
            ..self
        }
    }

    /// Specify the parameter to `start`. Defaults to `0`.
    pub const fn param(self, param: usize) -> Self {
        Self { param, ..self }
    }

    /// Specify the task's stack size.
    pub const fn stack_size(self, stack_size: usize) -> Self {
        assert!(
            self.stack.is_none(),
            "the task's stack is already specified"
        );

        Self {
            stack: Some(TaskStack::Auto(stack_size)),
            ..self
        }
    }

    /// Specify the task's hunk.
    pub const fn stack_hunk(self, stack_hunk: task::StackHunk<System>) -> Self {
        assert!(
            self.stack.is_none(),
            "the task's stack is already specified"
        );

        Self {
            stack: Some(TaskStack::Hunk(stack_hunk)),
            ..self
        }
    }

    /// [**Required**] Specify the task's initial base priority. Tasks with
    /// lower priority values execute first. The value must be in range
    /// `0..`[`num_task_priority_levels`].
    ///
    /// [`num_task_priority_levels`]: crate::kernel::cfg::CfgBuilder::num_task_priority_levels
    pub const fn priority(self, priority: usize) -> Self {
        Self {
            priority: Some(priority),
            ..self
        }
    }

    /// Specify whether the task should be activated at system startup.
    /// Defaults to `false` (don't activate).
    pub const fn active(self, active: bool) -> Self {
        Self { active, ..self }
    }

    /// Complete the definition of a task, returning a reference to the task.
    pub const fn finish(self, cfg: &mut CfgBuilder<System>) -> task::Task<System> {
        // FIXME: `Option::unwrap_or` is not `const fn` yet
        let stack = if let Some(stack) = self.stack {
            stack
        } else {
            TaskStack::Auto(System::STACK_DEFAULT_SIZE)
        };
        let stack = match stack {
            TaskStack::Auto(size) => {
                // Round up the stack size
                let size =
                    (size + System::STACK_ALIGN - 1) / System::STACK_ALIGN * System::STACK_ALIGN;

                let hunk = hunk::Hunk::<_, [_]>::build()
                    .len(size)
                    .align(System::STACK_ALIGN)
                    .zeroed()
                    .finish(cfg);

                // Safety: We just created a hunk just for this task, and we
                // don't use this hunk for other purposes.
                unsafe { task::StackHunk::from_hunk(hunk) }
            }
            TaskStack::Hunk(hunk) => hunk,
        };

        let inner = &mut cfg.inner;

        inner.tasks.push(CfgBuilderTask {
            // FIXME: Work-around for `Option::expect` being not `const fn`
            start: if let Some(x) = self.start {
                x
            } else {
                panic!("`start` (task entry point) is not specified")
            },
            param: self.param,
            stack,
            // FIXME: Work-around for `Option::expect` being not `const fn`
            priority: if let Some(x) = self.priority {
                x
            } else {
                panic!("`priority` is not specified")
            },
            active: self.active,
        });

        unsafe { task::Task::from_id(NonZeroUsize::new_unchecked(inner.tasks.len())) }
    }
}

#[doc(hidden)]
pub struct CfgBuilderTask<System> {
    start: fn(usize),
    param: usize,
    stack: task::StackHunk<System>,
    priority: usize,
    active: bool,
}

impl<System> Clone for CfgBuilderTask<System> {
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

impl<System> Copy for CfgBuilderTask<System> {}

impl<System: Port> CfgBuilderTask<System> {
    pub const fn to_state(&self, attr: &'static task::TaskAttr<System>) -> task::TaskCb<System> {
        // `self.priority` has already been checked by `to_attr`
        let priority = System::TASK_PRIORITY_LEVELS[self.priority];

        task::TaskCb {
            port_task_state: System::PORT_TASK_STATE_INIT,
            attr,
            base_priority: CpuLockCell::new(priority),
            effective_priority: CpuLockCell::new(priority),
            st: CpuLockCell::new(if self.active {
                task::TaskSt::PendingActivation
            } else {
                task::TaskSt::Dormant
            }),
            link: CpuLockCell::new(None),
            wait: Init::INIT,
            park_token: CpuLockCell::new(false),
            last_mutex_held: CpuLockCell::new(None),
        }
    }

    pub const fn to_attr(&self) -> task::TaskAttr<System> {
        task::TaskAttr {
            entry_point: self.start,
            entry_param: self.param,
            stack: self.stack,
            priority: if self.priority < System::NUM_TASK_PRIORITY_LEVELS {
                System::TASK_PRIORITY_LEVELS[self.priority]
            } else {
                panic!("task's `priority` must be less than `num_task_priority_levels`");
            },
        }
    }
}
