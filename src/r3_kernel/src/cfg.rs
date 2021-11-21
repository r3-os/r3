//! Static configuration mechanism for the kernel
use core::marker::PhantomData;

use r3::kernel::{cfg::KernelStatic, Hunk};

use crate::{
    utils::{ComptimeVec, FIXED_PRIO_BITMAP_MAX_LEN},
    KernelTraits, Port, System,
};

mod event_group;
mod interrupt;
mod mutex;
mod semaphore;
mod task;
mod timer;
pub use self::{event_group::*, interrupt::*, mutex::*, semaphore::*, task::*, timer::*};

/// Attach [a configuration function] to a "system" type by implementing
/// [`KernelCfg2`].
///
/// [a configuration function]: crate#static-configuration
/// [`KernelCfg2`]: crate::kernel::KernelCfg2
#[macro_export]
macro_rules! build {
    ($sys:ty, $configure:expr => $id_map_ty:ty) => {{
        use $crate::{
            cfg::{
                CfgBuilder, CfgBuilderInner, CfgBuilderInterruptHandler, InterruptHandlerFn,
                InterruptHandlerTable,
            },
            EventGroupCb, InterruptAttr, InterruptLineInit, KernelCfg1,
            KernelCfg2, Port, StartupHookAttr, State, TaskAttr, TaskCb, TimeoutRef, TimerAttr,
            TimerCb, SemaphoreCb, MutexCb, PortThreading, readyqueue,
            arrayvec::ArrayVec,
            utils::{
                for_times::U, AlignedStorage, FixedPrioBitmap, Init, RawCell, UIntegerWithBound,
            },
        };

        // `$configure` produces two values: a `CfgBuilder` and an ID map
        // (custom type). We need the first one to be `const` so that we can
        // calculate the values of generic parameters based on its contents.
        const CFG: CfgBuilderInner<$sys> = get_cfg();

        const fn get_cfg() -> CfgBuilderInner<$sys> {
            // FIXME: Unable to do this inside a `const` item because of
            //        <https://github.com/rust-lang/rust/pull/72934>

            // Safety: We are `build!`, so it's okay to use `CfgBuilder::new`
            let mut cfg = unsafe { CfgBuilder::new() };
            $configure(&mut cfg);
            cfg.finalize();
            cfg.into_inner()
        }

        // The second value can be just `let`
        // Safety: We are `build!`, so it's okay to use `CfgBuilder::new`
        const fn id_map() -> $id_map_ty {
            // FIXME: Unable to do this inside a `const` item because of
            //        <https://github.com/rust-lang/rust/pull/72934>
            //        This is also why `$id_map_ty` has to be given.

            $configure(&mut unsafe { CfgBuilder::new() })
        }

        // Set up task priority levels
        type TaskPriority = UIntegerWithBound<{ CFG.num_task_priority_levels as u128 - 1 }>;
        $crate::array_item_from_fn! {
            const TASK_PRIORITY_LEVELS: [TaskPriority; _] =
                (0..CFG.num_task_priority_levels).map(|i| i as _);
        };

        // Task ready queue
        type TaskReadyBitmap = FixedPrioBitmap<{ CFG.num_task_priority_levels }>;
        type TaskReadyQueue = readyqueue::BitmapQueue<
            $sys,
            <$sys as PortThreading>::PortTaskState,
            <$sys as KernelCfg1>::TaskPriority,
            TaskReadyBitmap,
            { CFG.num_task_priority_levels }
        >;

        // Safety: We are `build!`, so it's okay to `impl` this
        unsafe impl KernelCfg1 for $sys {
            const NUM_TASK_PRIORITY_LEVELS: usize = CFG.num_task_priority_levels;
            type TaskPriority = TaskPriority;
            type TaskReadyQueue = TaskReadyQueue;
            const TASK_PRIORITY_LEVELS: &'static [Self::TaskPriority] = &TASK_PRIORITY_LEVELS;
        }

        // Instantiiate task structures
        $crate::array_item_from_fn! {
            const TASK_ATTR_POOL: [TaskAttr<$sys>; _] =
                (0..CFG.tasks.len()).map(|i| CFG.tasks.get(i).to_attr());
            static TASK_CB_POOL:
                [TaskCb<$sys>; _] =
                    (0..CFG.tasks.len()).map(|i| CFG.tasks.get(i).to_state(&TASK_ATTR_POOL[i]));
        }

        // Instantiiate event group structures
        $crate::array_item_from_fn! {
            static EVENT_GROUP_CB_POOL:
                [EventGroupCb<$sys>; _] =
                    (0..CFG.event_groups.len()).map(|i| CFG.event_groups.get(i).to_state());
        }

        // Instantiiate mutex structures
        $crate::array_item_from_fn! {
            static MUTEX_CB_POOL:
                [MutexCb<$sys>; _] =
                    (0..CFG.mutexes.len()).map(|i| CFG.mutexes.get(i).to_state());
        }

        // Instantiiate semaphore structures
        $crate::array_item_from_fn! {
            static SEMAPHORE_CB_POOL:
                [SemaphoreCb<$sys>; _] =
                    (0..CFG.semaphores.len()).map(|i| CFG.semaphores.get(i).to_state());
        }

        // Instantiiate timer structures
        $crate::array_item_from_fn! {
            const TIMER_ATTR_POOL: [TimerAttr<$sys>; _] =
                (0..CFG.timers.len()).map(|i| CFG.timers.get(i).to_attr());
            static TIMER_CB_POOL:
                [TimerCb<$sys>; _] =
                    (0..CFG.timers.len()).map(|i| CFG.timers.get(i).to_state(&TIMER_ATTR_POOL[i], i));
        }

        // Instantiate hunks
        static HUNK_POOL: RawCell<AlignedStorage<{ CFG.hunk_pool_len }, { CFG.hunk_pool_align }>> =
            Init::INIT;

        // Instantiate the global state
        type KernelState = State<$sys>;
        static KERNEL_STATE: KernelState = State::INIT;

        // Consturct a table of combined second-level interrupt handlers
        const INTERRUPT_HANDLERS: [CfgBuilderInterruptHandler; { CFG.interrupt_handlers.len() }] =
            CFG.interrupt_handlers.to_array();
        const NUM_INTERRUPT_HANDLERS: usize = INTERRUPT_HANDLERS.len();
        const NUM_INTERRUPT_LINES: usize =
            $crate::kernel::cfg::num_required_interrupt_line_slots(&INTERRUPT_HANDLERS);
        struct Handlers;
        impl $crate::kernel::cfg::CfgBuilderInterruptHandlerList for Handlers {
            type NumHandlers = U<NUM_INTERRUPT_HANDLERS>;
            const HANDLERS: &'static [CfgBuilderInterruptHandler] = &INTERRUPT_HANDLERS;
        }
        const INTERRUPT_HANDLERS_SIZED: InterruptHandlerTable<
            [Option<InterruptHandlerFn>; NUM_INTERRUPT_LINES],
        > = unsafe {
            // Safety: (1) We are `build!`, so it's okay to call this.
            //         (2) `INTERRUPT_HANDLERS` contains at least
            //             `NUM_INTERRUPT_HANDLERS` elements.
            $crate::kernel::cfg::new_interrupt_handler_table::<
                $sys,
                U<NUM_INTERRUPT_LINES>,
                Handlers,
                NUM_INTERRUPT_LINES,
                NUM_INTERRUPT_HANDLERS,
            >()
        };

        // Construct a table of interrupt line initiializers
        $crate::array_item_from_fn! {
            const INTERRUPT_LINE_INITS:
                [InterruptLineInit<$sys>; _] =
                    (0..CFG.interrupt_lines.len()).map(|i| CFG.interrupt_lines.get(i).to_init());
        }

        // Construct a table of startup hooks
        $crate::array_item_from_fn! {
            const STARTUP_HOOKS:
                [StartupHookAttr; _] =
                    (0..CFG.startup_hooks.len()).map(|i| CFG.startup_hooks.get(i).to_attr());
        }

        // Calculate the required storage of the timeout heap
        const TIMEOUT_HEAP_LEN: usize = CFG.tasks.len() + CFG.timers.len();
        type TimeoutHeap = ArrayVec<TimeoutRef<$sys>, TIMEOUT_HEAP_LEN>;

        // Safety: We are `build!`, so it's okay to `impl` this
        unsafe impl KernelCfg2 for $sys {
            type TimeoutHeap = TimeoutHeap;

            #[inline(always)]
            fn state() -> &'static KernelState {
                &KERNEL_STATE
            }

            const INTERRUPT_HANDLERS: &'static InterruptHandlerTable = &INTERRUPT_HANDLERS_SIZED;

            const INTERRUPT_ATTR: InterruptAttr<Self> = InterruptAttr {
                _phantom: Init::INIT,
                line_inits: &INTERRUPT_LINE_INITS,
            };

            const STARTUP_HOOKS: &'static [StartupHookAttr] = &STARTUP_HOOKS;

            #[inline(always)]
            fn hunk_pool_ptr() -> *mut u8 {
                HUNK_POOL.get() as *mut u8
            }

            #[inline(always)]
            fn task_cb_pool() -> &'static [TaskCb<$sys>] {
                &TASK_CB_POOL
            }

            #[inline(always)]
            fn event_group_cb_pool() -> &'static [EventGroupCb<$sys>] {
                &EVENT_GROUP_CB_POOL
            }

            #[inline(always)]
            fn mutex_cb_pool() -> &'static [MutexCb<$sys>] {
                &MUTEX_CB_POOL
            }

            #[inline(always)]
            fn semaphore_cb_pool() -> &'static [SemaphoreCb<$sys>] {
                &SEMAPHORE_CB_POOL
            }

            #[inline(always)]
            fn timer_cb_pool() -> &'static [TimerCb<$sys>] {
                &TIMER_CB_POOL
            }
        }

        id_map()
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! array_item_from_fn {
    ($(
        $static_or_const:tt $out:ident: [$ty:ty; _] = (0..$len:expr).map(|$var:ident| $map:expr);
    )*) => {$(
        $static_or_const $out: [$ty; { $len }] = {
            use $crate::{core::mem::MaybeUninit, utils::mem};
            let mut values: [MaybeUninit<$ty>; { $len }] = mem::uninit_array();
            let mut i = 0;
            while i < $len {
                values[i] = MaybeUninit::<$ty>::new({
                    let $var = i;
                    $map
                });
                i += 1;
            }

            // Safety:  The memory layout of `[MaybeUninit<$ty>; $len]` is
            // identical to `[$ty; $len]`. We initialized all elements, so it's
            // safe to reinterpret that range as `[$ty; $len]`.
            unsafe { mem::transmute(values) }
        };
    )*};
}

/// A kernel configuration being constructed.
pub struct CfgBuilder<Traits: KernelTraits> {
    /// Disallows the mutation of `CfgBuilderInner` by a user-defined
    /// configuration function by making this not `pub`.
    inner: CfgBuilderInner<Traits>,
}

/// The private portion of [`CfgBuilder`]. This is not a real public interface,
/// but needs to be `pub` so [`build!`] can access the contents.
#[doc(hidden)]
pub struct CfgBuilderInner<Traits: KernelTraits> {
    _phantom: PhantomData<Traits>,
    pub hunk_pool_len: usize,
    pub hunk_pool_align: usize,
    pub tasks: ComptimeVec<CfgBuilderTask<Traits>>,
    pub num_task_priority_levels: usize,
    pub interrupt_lines: ComptimeVec<CfgBuilderInterruptLine>,
    pub startup_hook: Option<fn()>,
    pub event_groups: ComptimeVec<CfgBuilderEventGroup>,
    pub mutexes: ComptimeVec<CfgBuilderMutex>,
    pub semaphores: ComptimeVec<CfgBuilderSemaphore>,
    pub timers: ComptimeVec<CfgBuilderTimer>,
}

impl<Traits: KernelTraits> CfgBuilder<Traits> {
    /// Construct a `CfgBuilder`.
    ///
    /// # Safety
    ///
    /// This is only meant to be used by [`build!`]. For a particular system
    /// type, there can be only one fully-constructed instance of `CfgBuilder`,
    /// to which all defined kernel objects must belong. For example, swapping
    /// a given `CfgBuilder` with another one can be used to circumvent the
    /// compile-time access control of kernel objects.
    #[doc(hidden)]
    pub const unsafe fn new() -> Self {
        Self {
            inner: CfgBuilderInner {
                _phantom: PhantomData,
                hunk_pool_len: 0,
                hunk_pool_align: 1,
                tasks: ComptimeVec::new(),
                num_task_priority_levels: 4,
                interrupt_lines: ComptimeVec::new(),
                startup_hook: None,
                event_groups: ComptimeVec::new(),
                mutexes: ComptimeVec::new(),
                semaphores: ComptimeVec::new(),
                timers: ComptimeVec::new(),
            },
        }
    }

    /// Get `CfgBuilderInner`, consuming `self`.
    #[doc(hidden)]
    pub const fn into_inner(self) -> CfgBuilderInner<Traits> {
        self.inner
    }

    /// Apply post-processing before [`r3::kernel::Cfg`] is finalized.
    #[doc(hidden)]
    pub const fn finalize_in_cfg(cfg: &mut r3::kernel::Cfg<Self>)
    where
        Traits: KernelStatic,
    {
        // Create hunks for task stacks.
        let mut i = 0;
        let mut tasks = &mut cfg.raw().inner.tasks;
        while i < tasks.len() {
            if let Some(size) = tasks.get(i).stack.auto_size() {
                // Round up the stack size
                let size =
                    (size + Traits::STACK_ALIGN - 1) / Traits::STACK_ALIGN * Traits::STACK_ALIGN;

                let hunk = Hunk::build()
                    .len(size)
                    .align(Traits::STACK_ALIGN)
                    .finish(cfg);

                // Borrow again `tasks`, which was unborrowed because of the
                // call to `HunkDefiner::finish`
                tasks = &mut cfg.raw().inner.tasks;

                tasks.get_mut(i).stack = crate::task::StackHunk::from_hunk(hunk, size);
            }
            i += 1;
        }
    }
}

unsafe impl<Traits: KernelTraits> const r3::kernel::raw_cfg::CfgBase for CfgBuilder<Traits> {
    type System = System<Traits>;

    fn num_task_priority_levels(&mut self, new_value: usize) {
        if new_value == 0 {
            panic!("`num_task_priority_levels` must be greater than zero");
        } else if new_value > FIXED_PRIO_BITMAP_MAX_LEN {
            panic!("`num_task_priority_levels` must be less than or equal to `FIXED_PRIO_BITMAP_MAX_LEN`");
        } else if new_value >= isize::MAX as usize {
            // Limiting priority values in range `0..(isize::MAX - 1)`
            // leaves room for special values outside the extremities.
            //
            // This branch is actually unreachable because
            // `FIXED_PRIO_BITMAP_MAX_LEN` is so small compared to the size of
            // `isize`.
            unreachable!();
        }

        self.inner.num_task_priority_levels = new_value;
    }

    fn startup_hook_define(&mut self, func: fn()) {
        assert!(
            self.inner.startup_hook.is_none(),
            "only one combined startup hook can be registered"
        );
        self.inner.startup_hook = Some(func);
    }
}
