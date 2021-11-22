//! Tasks
use core::{fmt, hash};

use super::{
    cfg, raw, raw_cfg, ActivateTaskError, Cfg, GetCurrentTaskError, GetTaskPriorityError,
    InterruptTaskError, SetTaskPriorityError, UnparkError, UnparkExactError,
};
use crate::utils::{Init, PhantomInvariant};

// ----------------------------------------------------------------------------

/// Represents a single task in a system.
///
/// This type is ABI-compatible with [`Id`].
///
/// <div class="admonition-follows"></div>
///
/// > **Relation to Other Specifications:** Present in almost every real-time
/// > operating system.
///
/// # Task States
///
/// A task may be in one of the following states:
///
///  - **Dormant** — The task is not executing, doesn't have an associated
///    execution [thread], and can be [activated].
///
///  - **Ready** — The task has an associated execution thread, which is ready to
///    be scheduled to the CPU
///
///  - **Running** — The task has an associated execution thread, which is
///    currently scheduled to the CPU
///
///  - **Waiting** — The task has an associated execution thread, which is
///    currently blocked by a blocking operation
///
/// <center>
///
#[doc = svgbobdoc::transform_mdstr!(
/// ```svgbob
///                     .-------.
///    .--------------->| Ready |<--------------.
///    |                '-------'               |
///    |          dispatch | ^                  |
///    |                   | |                  |
///    | release           | |                  | activate
/// .---------.            | |           .---------.
/// | Waiting |            | |           | Dormant |
/// '---------'            | |           '---------'
///    ^                   | |                  ^
///    |                   | |                  |
///    |                   v | preempt          |
///    |          wait .---------.              |
///    '---------------| Running |--------------'
///                    '---------' exit
/// ```
)]
///
/// </center>
///
/// [thread]: crate#threads
/// [activated]: Task::activate
#[doc = include_str!("../common.md")]
#[repr(transparent)]
pub struct Task<System: raw::KernelBase>(System::TaskId);

impl<System: raw::KernelBase> Clone for Task<System> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<System: raw::KernelBase> Copy for Task<System> {}

impl<System: raw::KernelBase> PartialEq for Task<System> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<System: raw::KernelBase> Eq for Task<System> {}

impl<System: raw::KernelBase> hash::Hash for Task<System> {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        hash::Hash::hash(&self.0, state);
    }
}

impl<System: raw::KernelBase> fmt::Debug for Task<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Task").field(&self.0).finish()
    }
}

impl<System: raw::KernelBase> Task<System> {
    /// Construct a `Task` from `TaskId`.
    ///
    /// # Safety
    ///
    /// The kernel can handle invalid IDs without a problem. However, the
    /// constructed `Task` may point to an object that is not intended to be
    /// manipulated except by its creator. This is usually prevented by making
    /// `Task` an opaque handle, but this safeguard can be circumvented by
    /// this method.
    ///
    /// Constructing a `Task` for a current task is allowed. This can be safely
    /// done by [`Task::current`].
    pub const unsafe fn from_id(id: System::TaskId) -> Self {
        Self(id)
    }

    /// Get the raw `TaskId` value representing this task.
    pub const fn id(self) -> System::TaskId {
        self.0
    }
}

impl<System: raw::KernelBase> Task<System> {
    /// Construct a `CfgTaskBuilder` to define a mutex in [a configuration
    /// function](crate#static-configuration).
    pub const fn build() -> TaskDefiner<System> {
        TaskDefiner::new()
    }

    /// Get the current task (i.e., the task in the Running state).
    ///
    /// In a task context, this method returns the currently running task.
    ///
    /// In an interrupt context, the result is unreliable because scheduling is
    /// deferred until the control returns to a task, but the current interrupt
    /// handler could be interrupted by another interrrupt, which might do
    /// scheduling on return (whether this happens or not is unspecified).
    pub fn current() -> Result<Option<Self>, GetCurrentTaskError> {
        // Safety: "Constructing a `Task` for a current task is allowed."
        System::raw_task_current().map(|x| x.map(|id| unsafe { Self::from_id(id) }))
    }

    /// Start the execution of the task.
    pub fn activate(self) -> Result<(), ActivateTaskError> {
        // Safety: `Task` represents a permission to access the
        //         referenced object.
        unsafe { System::raw_task_activate(self.0) }
    }

    /// Interrupt any ongoing wait operations undertaken by the task.
    ///
    /// This method interrupt any ongoing system call that is blocking the task.
    /// The interrupted system call will return [`WaitError::Interrupted`] or
    /// [`WaitTimeoutError::Interrupted`].
    ///
    /// [`WaitError::Interrupted`]: crate::kernel::WaitError::Interrupted
    /// [`WaitTimeoutError::Interrupted`]: crate::kernel::WaitTimeoutError::Interrupted
    pub fn interrupt(self) -> Result<(), InterruptTaskError> {
        // Safety: `Task` represents a permission to access the
        //         referenced object.
        unsafe { System::raw_task_interrupt(self.0) }
    }

    /// Make the task's token available, unblocking [`Kernel::park`] now or in
    /// the future.
    ///
    /// If the token is already available, this method will return without doing
    /// anything. Use [`Task::unpark_exact`] if you need to detect this
    /// condition.
    ///
    /// If the task is currently being blocked by `Kernel::park`, the token will
    /// be immediately consumed. Otherwise, it will be consumed on a next call
    /// to `Kernel::park`.
    pub fn unpark(self) -> Result<(), UnparkError> {
        match self.unpark_exact() {
            Ok(()) | Err(UnparkExactError::QueueOverflow) => Ok(()),
            Err(UnparkExactError::BadContext) => Err(UnparkError::BadContext),
            Err(UnparkExactError::BadId) => Err(UnparkError::BadId),
            Err(UnparkExactError::BadObjectState) => Err(UnparkError::BadObjectState),
        }
    }

    /// Make *exactly* one new token available for the task, unblocking
    /// [`Kernel::park`] now or in the future.
    ///
    /// If the token is already available, this method will return
    /// [`UnparkExactError::QueueOverflow`]. Thus, this method will succeed
    /// only if it made *exactly* one token available.
    ///
    /// If the task is currently being blocked by `Kernel::park`, the token will
    /// be immediately consumed. Otherwise, it will be consumed on a next call
    /// to `Kernel::park`.
    pub fn unpark_exact(self) -> Result<(), UnparkExactError> {
        // Safety: `Task` represents a permission to access the
        //         referenced object.
        unsafe { System::raw_task_unpark_exact(self.0) }
    }

    /// Set the task's base priority.
    ///
    /// A task's base priority is used to calculate its [effective priority].
    /// Tasks with lower effective priorities execute first. The base priority
    /// is reset to the initial value specified by [`CfgTaskBuilder::priority`]
    /// upon activation.
    ///
    /// [effective priority]: Self::effective_priority
    /// [`CfgTaskBuilder::priority`]: crate::kernel::cfg::CfgTaskBuilder::priority
    ///
    /// The value must be in range `0..`[`num_task_priority_levels`]. Otherwise,
    /// this method will return [`SetTaskPriorityError::BadParam`].
    ///
    /// The task shouldn't be in the Dormant state. Otherwise, this method will
    /// return [`SetTaskPriorityError::BadObjectState`].
    ///
    /// [`num_task_priority_levels`]: crate::kernel::cfg::CfgBuilder::num_task_priority_levels
    pub fn set_priority(self, priority: usize) -> Result<(), SetTaskPriorityError>
    where
        System: raw::KernelTaskSetPriority,
    {
        // Safety: `Task` represents a permission to access the
        //         referenced object.
        unsafe { System::raw_task_set_priority(self.0, priority) }
    }

    /// Get the task's base priority.
    ///
    /// The task shouldn't be in the Dormant state. Otherwise, this method will
    /// return [`GetTaskPriorityError::BadObjectState`].
    pub fn priority(self) -> Result<usize, GetTaskPriorityError> {
        // Safety: `Task` represents a permission to access the
        //         referenced object.
        unsafe { System::raw_task_priority(self.0) }
    }

    /// Get the task's effective priority.
    ///
    /// The effective priority is calculated based on the task's [base priority]
    /// and can be temporarily raised by a [mutex locking protocol].
    ///
    /// [base priority]: Self::priority
    /// [mutex locking protocol]: crate::kernel::MutexProtocol
    ///
    /// The task shouldn't be in the Dormant state. Otherwise, this method will
    /// return [`GetTaskPriorityError::BadObjectState`].
    pub fn effective_priority(self) -> Result<usize, GetTaskPriorityError> {
        // Safety: `Task` represents a permission to access the
        //         referenced object.
        unsafe { System::raw_task_effective_priority(self.0) }
    }
}

// ----------------------------------------------------------------------------

/// The definer (static builder) for [`Task`].
#[must_use = "must call `finish()` to complete registration"]
pub struct TaskDefiner<System> {
    _phantom: PhantomInvariant<System>,
    start: Option<fn(usize)>,
    param: usize,
    stack_size: Option<usize>,
    priority: Option<usize>,
    active: bool,
}

impl<System: raw::KernelBase> TaskDefiner<System> {
    const fn new() -> Self {
        Self {
            _phantom: Init::INIT,
            start: None,
            param: 0,
            stack_size: None,
            priority: None,
            active: false,
        }
    }

    /// \[**Required**\] Specify the task's entry point.
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
            self.stack_size.is_none(),
            "the task's stack is already specified"
        );

        Self {
            stack_size: Some(stack_size),
            ..self
        }
    }

    // TODO: custom stack storage

    /// \[**Required**\] Specify the task's initial base priority. Tasks with
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

    /// Complete the definition of a task, returning a reference to the
    /// task.
    pub const fn finish<C: ~const raw_cfg::CfgTask<System = System>>(
        self,
        cfg: &mut Cfg<C>,
    ) -> Task<System> {
        let id = cfg.raw().task_define(
            raw_cfg::TaskDescriptor {
                phantom: Init::INIT,
                start: if let Some(x) = self.start {
                    x
                } else {
                    panic!("`start` (task entry point) is not specified")
                },
                param: self.param,
                active: self.active,
                priority: if let Some(x) = self.priority {
                    x
                } else {
                    panic!("`priority` (task entry point) is not specified")
                },
                stack_size: self.stack_size,
            },
            (),
        );
        unsafe { Task::from_id(id) }
    }
}

/// Specifies the [`Hunk`] to use as a task's stack when included in the task's
/// property [`Bag`].
///
/// A kernel might choose to ignore this if `StackHunk` is not supported.
///
/// If a `StackHunk` is given, the stack size ([`TaskDefiner::stack_size`]) must
/// be specified explicitly.
///
/// [`Bag`]: crate::bag::Bag
/// [`Hunk`]: crate::kernel::Hunk
pub struct StackHunk<System: cfg::KernelStatic>(super::Hunk<System>);

impl<System: cfg::KernelStatic> StackHunk<System> {
    /// Construct `StackHunk`.
    ///
    /// # Safety
    ///
    /// When activating the assocaited task, the kernel will mutably borrow
    /// the region starting at `hunk` without no borrow checking.
    pub const unsafe fn new(hunk: super::Hunk<System>) -> Self {
        Self(hunk)
    }

    /// Get the contained [`Hunk`].
    #[inline]
    pub const fn hunk(self) -> super::Hunk<System> {
        self.0
    }
}

impl<System: cfg::KernelStatic> Clone for StackHunk<System> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<System: cfg::KernelStatic> Copy for StackHunk<System> {}
