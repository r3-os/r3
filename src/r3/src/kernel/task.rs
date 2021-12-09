//! Tasks
use core::{fmt, hash};

use raw::KernelBase;

use super::{
    cfg, raw, raw_cfg, ActivateTaskError, Cfg, GetCurrentTaskError, GetTaskPriorityError,
    InterruptTaskError, SetTaskPriorityError, UnparkError, UnparkExactError,
};
use crate::utils::{Init, PhantomInvariant};

// ----------------------------------------------------------------------------

define_object! {
/// Represents a single task in a system.
///
/// This type is ABI-compatible with `System::`[`RawTaskId`][].
///
/// <div class="admonition-follows"></div>
///
/// > **Relation to Other Specifications:** Present in almost every real-time
/// > operating system.
///
/// [`RawTaskId`]: raw::KernelBase::RawTaskId
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
/// [activated]: TaskMethods::activate
#[doc = include_str!("../common.md")]
pub struct Task<System: _>(System::RawTaskId);

/// Represents a single borrowed task in a system.
#[doc = include_str!("../common.md")]
pub struct TaskRef<System: raw::KernelBase>(_);

pub trait TaskHandle {}
pub trait TaskMethods {}
}

impl<System: raw::KernelBase> TaskRef<'_, System> {
    /// Construct a `TaskDefiner` to define a mutex in [a configuration
    /// function](crate#static-configuration).
    pub const fn define() -> TaskDefiner<System> {
        TaskDefiner::new()
    }
}

/// The supported operations on [`TaskHandle`].
#[doc = include_str!("../common.md")]
pub trait TaskMethods: TaskHandle {
    // TODO: Make `current` actually safe
    /// Get the current task (i.e., the task in the Running state).
    ///
    /// In a task context, this method returns the currently running task.
    ///
    /// In an interrupt context, the result is unreliable because scheduling is
    /// deferred until the control returns to a task, but the current interrupt
    /// handler could be interrupted by another interrrupt, which might do
    /// scheduling on return (whether this happens or not is unspecified).
    #[inline]
    fn current() -> Result<Option<TaskRef<'static, Self::System>>, GetCurrentTaskError> {
        // Safety: "Constructing a `Task` for a current task is allowed."
        <Self::System as KernelBase>::raw_task_current()
            .map(|x| x.map(|id| unsafe { TaskRef::from_id(id) }))
    }

    /// Start the execution of the task.
    #[inline]
    fn activate(&self) -> Result<(), ActivateTaskError> {
        // Safety: `Task` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as KernelBase>::raw_task_activate(self.id()) }
    }

    /// Interrupt any ongoing wait operations undertaken by the task.
    ///
    /// This method interrupt any ongoing system call that is blocking the task.
    /// The interrupted system call will return [`WaitError::Interrupted`] or
    /// [`WaitTimeoutError::Interrupted`].
    ///
    /// [`WaitError::Interrupted`]: crate::kernel::WaitError::Interrupted
    /// [`WaitTimeoutError::Interrupted`]: crate::kernel::WaitTimeoutError::Interrupted
    #[inline]
    fn interrupt(&self) -> Result<(), InterruptTaskError> {
        // Safety: `Task` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as KernelBase>::raw_task_interrupt(self.id()) }
    }

    /// Make the task's token available, unblocking [`Kernel::park`][] now or in
    /// the future.
    ///
    /// If the token is already available, this method will return without doing
    /// anything. Use [`Self::unpark_exact`] if you need to detect this
    /// condition.
    ///
    /// If the task is currently being blocked by `Kernel::park`, the token will
    /// be immediately consumed. Otherwise, it will be consumed on a next call
    /// to `Kernel::park`.
    ///
    /// [`Kernel::park`]: crate::kernel::Kernel::park
    #[inline]
    fn unpark(&self) -> Result<(), UnparkError> {
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
    ///
    /// [`Kernel::park`]: crate::kernel::Kernel::park
    #[inline]
    fn unpark_exact(&self) -> Result<(), UnparkExactError> {
        // Safety: `Task` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as KernelBase>::raw_task_unpark_exact(self.id()) }
    }

    /// Set the task's base priority.
    ///
    /// A task's base priority is used to calculate its [effective priority].
    /// Tasks with lower effective priorities execute first. The base priority
    /// is reset to the initial value specified by [`TaskDefiner::priority`]
    /// upon activation.
    ///
    /// [effective priority]: Self::effective_priority
    /// [`TaskDefiner::priority`]: crate::kernel::task::TaskDefiner::priority
    ///
    /// The value must be in range `0..`[`num_task_priority_levels`]. Otherwise,
    /// this method will return [`SetTaskPriorityError::BadParam`].
    ///
    /// The task shouldn't be in the Dormant state. Otherwise, this method will
    /// return [`SetTaskPriorityError::BadObjectState`].
    ///
    /// [`num_task_priority_levels`]: crate::kernel::Cfg::num_task_priority_levels
    #[inline]
    fn set_priority(&self, priority: usize) -> Result<(), SetTaskPriorityError>
    where
        Self::System: raw::KernelTaskSetPriority,
    {
        // Safety: `Task` represents a permission to access the
        //         referenced object.
        unsafe {
            <Self::System as raw::KernelTaskSetPriority>::raw_task_set_priority(self.id(), priority)
        }
    }

    /// Get the task's base priority.
    ///
    /// The task shouldn't be in the Dormant state. Otherwise, this method will
    /// return [`GetTaskPriorityError::BadObjectState`].
    #[inline]
    fn priority(&self) -> Result<usize, GetTaskPriorityError> {
        // Safety: `Task` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as raw::KernelBase>::raw_task_priority(self.id()) }
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
    #[inline]
    fn effective_priority(&self) -> Result<usize, GetTaskPriorityError> {
        // Safety: `Task` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as raw::KernelBase>::raw_task_effective_priority(self.id()) }
    }
}

impl<T: TaskHandle> TaskMethods for T {}

// ----------------------------------------------------------------------------

/// The definer (static builder) for [`TaskRef`].
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
    /// [`num_task_priority_levels`]: crate::kernel::Cfg::num_task_priority_levels
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
    ) -> TaskRef<'static, System> {
        let id = cfg.raw().task_define(
            raw_cfg::TaskDescriptor {
                phantom: Init::INIT,
                start: self
                    .start
                    .expect("`start` (task entry point) is not specified"),
                param: self.param,
                active: self.active,
                priority: self
                    .priority
                    .expect("`priority` (task entry point) is not specified"),
                stack_size: self.stack_size,
            },
            (),
        );
        unsafe { TaskRef::from_id(id) }
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

    /// Get the referenced [`Hunk`].
    ///
    /// [`Hunk`]: crate::kernel::Hunk
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
