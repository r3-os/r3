//! Static configuration mechanism for the kernel
use core::marker::PhantomData;

use r3::kernel::Hunk;

use crate::{
    utils::{ComptimeVec, FIXED_PRIO_BITMAP_MAX_LEN},
    KernelTraits, System,
};

mod event_group;
mod interrupt;
mod mutex;
mod semaphore;
mod task;
mod timer;
pub use self::{event_group::*, interrupt::*, mutex::*, semaphore::*, task::*, timer::*};

/// Attach [a configuration function][1] to a [kernel trait type][2] by
/// implementing [`KernelCfg2`].
///
/// [1]: r3#static-configuration
/// [2]: crate#kernel-trait-type
/// [`KernelCfg2`]: crate::KernelCfg2
#[macro_export]
macro_rules! build {
    // `$configure: ~const Fn(&mut Cfg<impl ~const CfgBase<System =
    // r3_kernel::System<$Traits>>) -> $IdMap`
    ($Traits:ty, $configure:expr => $IdMap:ty) => {{
        use $crate::{
            r3,
            cfg::{self, CfgBuilder, CfgBuilderInner},
            EventGroupCb, InterruptAttr, InterruptLineInit, KernelCfg1,
            KernelCfg2, Port, State, TaskAttr, TaskCb, TimeoutRef, TimerAttr,
            TimerCb, SemaphoreCb, MutexCb, PortThreading, readyqueue,
            arrayvec::ArrayVec,
            utils::{
                AlignedStorage, FixedPrioBitmap, Init, RawCell, UIntegerWithBound,
            },
        };

        type System = $crate::System<$Traits>;

        const fn build_cfg_pre() -> r3::kernel::cfg::KernelStaticParams<System> {
            // Safety: We are `build!`, so it's okay to use `CfgBuilder::new`
            let mut my_cfg = unsafe { CfgBuilder::new() };
            let mut cfg = r3::kernel::cfg::Cfg::new(&mut my_cfg);
            $configure(&mut cfg);
            CfgBuilder::finalize_in_cfg(&mut cfg);

            // Get `KernelStaticParams`, which is necessary for the later phases
            // of the finalization. Throw away `my_cfg` for now.
            cfg.finish_pre()
        }

        // Implement `KernelStatic` on `$Traits` using the information
        // collected in the first part of the finalization
        r3::kernel::cfg::attach_static!(
            build_cfg_pre(),
            impl KernelStatic<System> for $Traits,
        );

        // The later part of the finalization continues using the
        // `KernelStatic` implementation
        const fn build_cfg_post() -> (CfgBuilderInner<$Traits>, $IdMap) {
            // Safety: We are `build!`, so it's okay to use `CfgBuilder::new`
            let mut my_cfg = unsafe { CfgBuilder::new() };
            let mut cfg = r3::kernel::cfg::Cfg::new(&mut my_cfg);
            let id_map = $configure(&mut cfg);
            CfgBuilder::finalize_in_cfg(&mut cfg);

            // Throw away the returned `KernelStaticParams` because we already
            // have one and used it for the first phase
            cfg.finish_pre();

            // Complete the finalization. This makes the final changes to
            // `my_cfg`
            cfg.finish_interrupt();
            cfg.finish_post();

            (my_cfg.into_inner(), id_map)
        }

        const CFG_OUTPUT: (CfgBuilderInner<$Traits>, $IdMap) = build_cfg_post();
        const CFG: CfgBuilderInner<$Traits> = CFG_OUTPUT.0;

        // Set up task priority levels
        type TaskPriority = UIntegerWithBound<{ CFG.num_task_priority_levels as u128 - 1 }>;
        $crate::array_item_from_fn! {
            const TASK_PRIORITY_LEVELS: [TaskPriority; _] =
                (0..CFG.num_task_priority_levels).map(|i| i as _);
        };

        // Task ready queue
        type TaskReadyBitmap = FixedPrioBitmap<{ CFG.num_task_priority_levels }>;
        type TaskReadyQueue = readyqueue::BitmapQueue<
            $Traits,
            <$Traits as PortThreading>::PortTaskState,
            <$Traits as KernelCfg1>::TaskPriority,
            TaskReadyBitmap,
            { CFG.num_task_priority_levels }
        >;

        // Safety: We are `build!`, so it's okay to `impl` this
        unsafe impl KernelCfg1 for $Traits {
            const NUM_TASK_PRIORITY_LEVELS: usize = CFG.num_task_priority_levels;
            type TaskPriority = TaskPriority;
            type TaskReadyQueue = TaskReadyQueue;
            const TASK_PRIORITY_LEVELS: &'static [Self::TaskPriority] = &TASK_PRIORITY_LEVELS;
        }

        // Instantiiate task structures
        $crate::array_item_from_fn! {
            const TASK_ATTR_POOL: [TaskAttr<$Traits>; _] =
                (0..CFG.tasks.len()).map(|i| CFG.tasks[i].to_attr());
            static TASK_CB_POOL:
                [TaskCb<$Traits>; _] =
                    (0..CFG.tasks.len()).map(|i| CFG.tasks[i].to_state(&TASK_ATTR_POOL[i]));
        }

        // Instantiiate event group structures
        $crate::array_item_from_fn! {
            static EVENT_GROUP_CB_POOL:
                [EventGroupCb<$Traits>; _] =
                    (0..CFG.event_groups.len()).map(|i| CFG.event_groups[i].to_state());
        }

        // Instantiiate mutex structures
        $crate::array_item_from_fn! {
            static MUTEX_CB_POOL:
                [MutexCb<$Traits>; _] =
                    (0..CFG.mutexes.len()).map(|i| CFG.mutexes[i].to_state());
        }

        // Instantiiate semaphore structures
        $crate::array_item_from_fn! {
            static SEMAPHORE_CB_POOL:
                [SemaphoreCb<$Traits>; _] =
                    (0..CFG.semaphores.len()).map(|i| CFG.semaphores[i].to_state());
        }

        // Instantiiate timer structures
        $crate::array_item_from_fn! {
            const TIMER_ATTR_POOL: [TimerAttr<$Traits>; _] =
                (0..CFG.timers.len()).map(|i| CFG.timers[i].to_attr());
            static TIMER_CB_POOL:
                [TimerCb<$Traits>; _] =
                    (0..CFG.timers.len()).map(|i| CFG.timers[i].to_state(&TIMER_ATTR_POOL[i], i));
        }

        // Instantiate hunks
        static HUNK_POOL: RawCell<AlignedStorage<{ CFG.hunk_pool_len }, { CFG.hunk_pool_align }>> =
            Init::INIT;

        // Instantiate the global state
        type KernelState = State<$Traits>;
        static KERNEL_STATE: KernelState = State::INIT;

        // Construct a table of interrupt handlers
        const INTERRUPT_HANDLER_TABLE_LEN: usize =
            cfg::interrupt_handler_table_len(CFG.interrupt_lines.as_slice());
        const INTERRUPT_HANDLER_TABLE:
            [Option<r3::kernel::interrupt::InterruptHandlerFn>; INTERRUPT_HANDLER_TABLE_LEN] =
            cfg::interrupt_handler_table(CFG.interrupt_lines.as_slice());

        // Construct a table of interrupt line initiializers
        $crate::array_item_from_fn! {
            const INTERRUPT_LINE_INITS:
                [InterruptLineInit; _] =
                    (0..CFG.interrupt_lines.len()).map(|i| CFG.interrupt_lines[i].to_init());
        }

        // Calculate the required storage of the timeout heap
        const TIMEOUT_HEAP_LEN: usize = CFG.tasks.len() + CFG.timers.len();
        type TimeoutHeap = ArrayVec<TimeoutRef<$Traits>, TIMEOUT_HEAP_LEN>;

        #[inline]
        unsafe fn no_startup_hook() {}

        // Safety: We are `build!`, so it's okay to `impl` this
        unsafe impl KernelCfg2 for $Traits {
            type TimeoutHeap = TimeoutHeap;

            #[inline(always)]
            fn state() -> &'static KernelState {
                &KERNEL_STATE
            }

            const INTERRUPT_HANDLERS: &'static cfg::InterruptHandlerTable = &cfg::InterruptHandlerTable {
                storage: &INTERRUPT_HANDLER_TABLE,
            };

            const INTERRUPT_ATTR: InterruptAttr<Self> = InterruptAttr {
                _phantom: Default::default(),
                line_inits: &INTERRUPT_LINE_INITS,
            };

            const STARTUP_HOOK: unsafe fn() = if let Some(x) = CFG.startup_hook {
                x
            } else {
                no_startup_hook
            };

            #[inline(always)]
            fn hunk_pool_ptr() -> *mut u8 {
                HUNK_POOL.get() as *mut u8
            }

            #[inline(always)]
            fn task_cb_pool() -> &'static [TaskCb<$Traits>] {
                &TASK_CB_POOL
            }

            #[inline(always)]
            fn event_group_cb_pool() -> &'static [EventGroupCb<$Traits>] {
                &EVENT_GROUP_CB_POOL
            }

            #[inline(always)]
            fn mutex_cb_pool() -> &'static [MutexCb<$Traits>] {
                &MUTEX_CB_POOL
            }

            #[inline(always)]
            fn semaphore_cb_pool() -> &'static [SemaphoreCb<$Traits>] {
                &SEMAPHORE_CB_POOL
            }

            #[inline(always)]
            fn timer_cb_pool() -> &'static [TimerCb<$Traits>] {
                &TIMER_CB_POOL
            }
        }

        CFG_OUTPUT.1
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
    pub const fn finalize_in_cfg(cfg: &mut r3::kernel::Cfg<Self>) {
        // Create hunks for task stacks.
        let mut i = 0;
        let mut tasks = &mut cfg.raw().inner.tasks;
        while i < tasks.len() {
            if let Some(size) = tasks[i].stack.auto_size() {
                // Round up the stack size
                let size =
                    (size + Traits::STACK_ALIGN - 1) / Traits::STACK_ALIGN * Traits::STACK_ALIGN;

                let hunk = Hunk::define()
                    .len(size)
                    .align(Traits::STACK_ALIGN)
                    .finish(cfg);

                // Borrow again `tasks`, which was unborrowed because of the
                // call to `HunkDefiner::finish`
                tasks = &mut cfg.raw().inner.tasks;

                tasks[i].stack = crate::task::StackHunk::from_hunk(hunk, size);
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
