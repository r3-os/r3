//! Task ready queue implementation (internal use only).
//!
//! **This module is exempt from the API stability guarantee.**
use crate::{
    kernel::{
        task::TaskCb,
        utils::{CpuLockCell, CpuLockGuardBorrowMut},
        Kernel, KernelCfg1, PortThreading,
    },
    utils::{
        intrusive_list::{Ident, ListAccessorCell, Static, StaticLink, StaticListHead},
        Init, PrioBitmap,
    },
};
use core::{fmt, ops::RangeTo};
use num_traits::ToPrimitive;

/// Represents a task ready queue, which tracks a list of Ready tasks, sorted by
/// effective priority order.
///
/// This trait is not intended to be implemented on custom types.
pub trait Queue<System>: Send + Sync + fmt::Debug + Init + 'static + private::Sealed {
    type PerTaskData: Send + Sync + fmt::Debug + Init + 'static;

    /// Return a flag indicating whether there's a task in Ready state whose
    /// priority is in the specified range.
    fn has_ready_task_in_priority_range(&self, ctx: Ctx<'_, System>, range: RangeTo<usize>) -> bool
    where
        System: Kernel;

    /// Insert the specified task `task_cb` to the ready queue.
    ///
    /// `task_cb` will be inserted as close to the back as possible without
    /// violating the priority ordering. I.e., if there are one or more tasks
    /// having effective priorities identical to that of `task_cb`, `task_cb`
    /// will be inserted after such tasks.
    fn push_back_task(&self, ctx: Ctx<'_, System>, task_cb: &'static TaskCb<System>)
    where
        System: Kernel;

    /// Choose the next task to schedule based on `prev_task_priority`, the
    /// priority of the current task (more precisely, the task that would run
    /// after the ongoing scheduling decision if preemption was not requested by
    /// this decision). If there's no such current task, `prev_task_priority`
    /// should be `usize::MAX`, in which case this method will return
    /// `SwitchTo(_)`.
    ///
    /// This method performs the following abstract steps:
    ///
    ///  1. If `prev_task_priority` does not equal to `usize::MAX`, insert
    ///     an imaginary task with that effective priority into the ready queue
    ///     as close to the front as possible without violating the priority
    ///     ordering. This imaginary task only exists during the duration of
    ///     the current method call.
    ///
    ///  2. If the ready queue is empty, return `SwitchTo(None)`.
    ///
    ///  3. Pop a task from the front of the ready queue.
    ///
    ///  4. If the popped task `t` is the imaginary task inserted in step 1,
    ///     return `Keep`. Otherwise, return `SwitchTo(t)`.
    ///
    /// | Has current task? | Is it blocked? | `prev_task_priority` | Has next task? |        Returns      |
    /// | ----------------- | -------------- | -------------------- | -------------- | ------------------- |
    /// |        no         |       no       |   `== usize::MAX`    |       no       |  `SwitchTo(None)`   |
    /// |        no         |       no       |   `== usize::MAX`    |       yes      | `SwitchTo(Some(_))` |
    /// |        yes        |       yes      |   `== usize::MAX`    |       no       |  `SwitchTo(None)`   |
    /// |        yes        |       yes      |   `== usize::MAX`    |       yes      | `SwitchTo(Some(_))` |
    /// |        yes        |       no       |   `!= usize::MAX`    |       no       |       `Keep`        |
    /// |        yes        |       no       |   `!= usize::MAX`    |       yes      | `SwitchTo(Some(_))` |
    ///
    ///  - *Has current task?* and *Is it blocked?* columns are contexts in
    ///    which this method is called but are not directly observable by this
    ///    method's implementation.
    ///
    ///  - `prev_task_priority` is the value passed to this method.
    ///
    ///  - *Has next task?* column is a possible outcome of the scheduling
    ///    decision made by this method.
    ///
    ///  - *Returns* column indicates what this method is supposed to return in
    ///    the respective cases.
    ///
    fn pop_front_task(
        &self,
        ctx: Ctx<'_, System>,
        prev_task_priority: usize,
    ) -> ScheduleDecision<&'static TaskCb<System>>
    where
        System: Kernel;

    /// Reposition the specified task within the ready queue after a change in
    /// its effective priority from `old_effective_priority` to
    /// `effective_priority`.
    ///
    /// `task_cb` will be re-inserted as close to the back as possible without
    /// violating the priority ordering. I.e., if there are one or more tasks
    /// having effective priorities identical to that of `task_cb`, `task_cb`
    /// will be re-inserted after such tasks.
    ///
    /// The caller should ensure `old_effective_priority` is not identical to
    /// `effective_priority`.
    fn reorder_task(
        &self,
        ctx: Ctx<'_, System>,
        task_cb: &'static TaskCb<System>,
        effective_priority: usize,
        old_effective_priority: usize,
    ) where
        System: Kernel;
}

/// Implements [the sealed trait pattern], which prevents [`Queue`] against
/// downstream implementations.
///
/// [the sealed trait pattern]: https://rust-lang.github.io/api-guidelines/future-proofing.html
mod private {
    pub trait Sealed {}
}

/// The result type of [`Queue::pop_front_task`].
pub enum ScheduleDecision<T> {
    /// The kernel should not perform context switch and should continue to
    /// schedule the current task.
    Keep,
    /// The kernel should perform context switch to the specified task.
    SwitchTo(Option<T>),
}

/// The context type for [`Queue`].
pub struct Ctx<'a, System: Kernel> {
    pub(super) lock: CpuLockGuardBorrowMut<'a, System>,
}

impl<'a, System: Kernel> From<CpuLockGuardBorrowMut<'a, System>> for Ctx<'a, System> {
    #[inline]
    fn from(lock: CpuLockGuardBorrowMut<'a, System>) -> Self {
        Self { lock }
    }
}

/// The ready queue implementation that uses a set of queues segregated by the
/// priorities of contained tasks.
pub struct BitmapQueue<
    System: PortThreading,
    PortTaskState: 'static,
    TaskPriority: 'static,
    Bitmap: 'static,
    const LEN: usize,
> {
    /// The set of segregated task ready queues, in which each queue stores
    /// the list of Ready tasks at the corresponding priority.
    ///
    /// Invariant: `queues[i].first.is_some() == bitmap.get(i)`
    queues: [CpuLockCell<
        System,
        StaticListHead<BitmapQueueTaskCb<System, PortTaskState, TaskPriority>>,
    >; LEN],

    /// The task ready bitmap, in which each bit indicates whether the
    /// segregated queue corresponding to that bit contains a task or not.
    bitmap: CpuLockCell<System, Bitmap>,
}

impl<
        System: PortThreading,
        PortTaskState: 'static,
        TaskPriority: 'static,
        Bitmap: 'static + Init,
        const LEN: usize,
    > Init for BitmapQueue<System, PortTaskState, TaskPriority, Bitmap, LEN>
{
    const INIT: Self = Self {
        queues: Init::INIT,
        bitmap: Init::INIT,
    };
}

type BitmapQueueTaskCb<System, PortTaskState, TaskPriority> = TaskCb<
    System,
    PortTaskState,
    TaskPriority,
    BitmapQueuePerTaskData<System, PortTaskState, TaskPriority>,
>;

pub struct BitmapQueuePerTaskData<
    System: PortThreading,
    PortTaskState: 'static,
    TaskPriority: 'static,
> {
    link: CpuLockCell<
        System,
        Option<StaticLink<BitmapQueueTaskCb<System, PortTaskState, TaskPriority>>>,
    >,
}

impl<System: PortThreading, PortTaskState: 'static, TaskPriority: 'static> Init
    for BitmapQueuePerTaskData<System, PortTaskState, TaskPriority>
{
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self { link: Init::INIT };
}

impl<System: Kernel, PortTaskState: 'static, TaskPriority: 'static> fmt::Debug
    for BitmapQueuePerTaskData<System, PortTaskState, TaskPriority>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BitmapQueuePerTaskData")
            .field("link", &self.link)
            .finish()
    }
}

/// Get a `ListAccessorCell` used to access a task ready queue.
macro_rules! list_accessor {
    ($head:expr, $key:expr) => {
        ListAccessorCell::new(
            $head,
            &Static,
            |task_cb| &task_cb.ready_queue_data.link,
            $key,
        )
    };
}

impl<System: Kernel, Bitmap: PrioBitmap, const LEN: usize> Queue<System>
    for BitmapQueue<
        System,
        <System as PortThreading>::PortTaskState,
        <System as KernelCfg1>::TaskPriority,
        Bitmap,
        LEN,
    >
where
    System: KernelCfg1<TaskReadyQueue = Self>,
{
    type PerTaskData = BitmapQueuePerTaskData<
        System,
        <System as PortThreading>::PortTaskState,
        <System as KernelCfg1>::TaskPriority,
    >;

    #[inline]
    fn has_ready_task_in_priority_range(
        &self,
        Ctx { lock }: Ctx<'_, System>,
        range: RangeTo<usize>,
    ) -> bool {
        let highest_task_priority = self.bitmap.read(&*lock).find_set().unwrap_or(usize::MAX);
        highest_task_priority < range.end
    }

    #[inline]
    fn push_back_task(&self, Ctx { mut lock }: Ctx<'_, System>, task_cb: &'static TaskCb<System>) {
        // Insert the task to a ready queue
        let pri = task_cb.effective_priority.read(&*lock).to_usize().unwrap();
        list_accessor!(&self.queues[pri], lock.borrow_mut()).push_back(Ident(task_cb));

        // Update `bitmap` accordingly
        self.bitmap.write(&mut *lock).set(pri);
    }

    #[inline]
    fn pop_front_task(
        &self,
        Ctx { mut lock }: Ctx<'_, System>,
        prev_task_priority: usize,
    ) -> ScheduleDecision<&'static TaskCb<System>> {
        // The priority of the next task to run
        //
        // Consider the case where `prev_task_priority == usize::MAX`, i.e.,
        // there is no current task.
        //
        // The default value (the value given to `unwrap_or`) is
        // `usize::MAX - 1` for the following reason:
        // If there's no task to schedule at the moment, this method is supposed
        // to return `SwitchTo(None)`.  If the default value was `usize::MAX`,
        // in this case, `prev_task_priority` would be equal to
        // `next_task_priority` and this method would return `Keep`. We make
        // sure this does not happen by making the default value lower.
        //
        // `usize::MAX - 1` never collides with an actual task priority because
        // of the priority range restriction imposed by `CfgBuilder::
        // num_task_priority_levels`.
        let next_task_priority = self
            .bitmap
            .read(&*lock)
            .find_set()
            .unwrap_or(usize::MAX - 1);

        if prev_task_priority <= next_task_priority {
            // Return if there's no task willing to take over the current one,
            // and the current one can still run.
            ScheduleDecision::Keep
        } else if next_task_priority < LEN {
            // Take the first task from the ready queue corresponding to
            // `next_task_priority`
            let mut accessor = list_accessor!(&self.queues[next_task_priority], lock.borrow_mut());
            let task = accessor.pop_front().unwrap().0;

            // Update `bitmap` accordingly
            if accessor.is_empty() {
                self.bitmap.write(&mut *lock).clear(next_task_priority);
            }

            ScheduleDecision::SwitchTo(Some(task))
        } else {
            ScheduleDecision::SwitchTo(None)
        }
    }

    #[inline]
    fn reorder_task(
        &self,
        Ctx { mut lock }: Ctx<'_, System>,
        task_cb: &'static TaskCb<System>,
        effective_priority: usize,
        old_effective_priority: usize,
    ) {
        debug_assert_ne!(effective_priority, old_effective_priority);

        // Move the task between ready queues
        let old_pri_empty = {
            let mut accessor =
                list_accessor!(&self.queues[old_effective_priority], lock.borrow_mut());
            accessor.remove(Ident(task_cb));
            accessor.is_empty()
        };

        list_accessor!(&self.queues[effective_priority], lock.borrow_mut())
            .push_back(Ident(task_cb));

        // Update `bitmap` accordingly
        // (This code assumes `effective_priority != old_effective_priority`.)
        let task_ready_bitmap = self.bitmap.write(&mut *lock);
        task_ready_bitmap.set(effective_priority);
        if old_pri_empty {
            task_ready_bitmap.clear(old_effective_priority);
        }
    }
}

impl<
        System: Kernel,
        PortTaskState: 'static,
        TaskPriority: 'static,
        Bitmap: 'static + fmt::Debug,
        const LEN: usize,
    > fmt::Debug for BitmapQueue<System, PortTaskState, TaskPriority, Bitmap, LEN>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Ok(lock) = super::utils::lock_cpu() {
            let lock = core::cell::RefCell::new(lock);
            let lock = &lock; // capture-by-reference in the closure below

            struct DebugFn<F>(F);
            impl<F: Fn(&mut fmt::Formatter) -> fmt::Result> fmt::Debug for DebugFn<F> {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    (self.0)(f)
                }
            }

            f.write_str("BitmapQueue ")?;
            f.debug_map()
                .entries(self.queues.iter().enumerate().map(|(i, head_cell)| {
                    (
                        // key = priority
                        i,
                        // value = list of tasks
                        DebugFn(move |f: &mut fmt::Formatter| {
                            let mut lock = lock.borrow_mut();
                            let accessor = list_accessor!(head_cell, lock.borrow_mut());
                            f.debug_list()
                                .entries(accessor.iter().map(|x| x.0))
                                .finish()
                        }),
                    )
                }))
                .finish()
        } else {
            f.write_str("BitmapQueue { < locked > }")
        }
    }
}

impl<System: Kernel, Bitmap: PrioBitmap, const LEN: usize> private::Sealed
    for BitmapQueue<
        System,
        <System as PortThreading>::PortTaskState,
        <System as KernelCfg1>::TaskPriority,
        Bitmap,
        LEN,
    >
where
    System: KernelCfg1<TaskReadyQueue = Self>,
{
}
