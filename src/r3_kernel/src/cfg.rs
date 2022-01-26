//! Static configuration mechanism for the kernel
use r3_core::{kernel::Hunk, utils::ConstAllocator};

use crate::{
    utils::{ComptimeVec, Frozen, FIXED_PRIO_BITMAP_MAX_LEN},
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
/// [1]: r3_core#static-configuration
/// [2]: crate#kernel-trait-type
/// [`KernelCfg2`]: crate::KernelCfg2
#[macro_export]
macro_rules! build {
    // `$configure: ~const Fn(&mut Cfg<impl ~const CfgBase<System =
    // r3_kernel::System<$Traits>>) -> $IdMap`
    ($Traits:ty, $configure:expr => $IdMap:ty) => {{
        use $crate::{
            r3_core::{
                self,
                utils::ConstAllocator,
            },
            cfg::{self, CfgBuilder, MiddleCfg},
            EventGroupCb, InterruptAttr, InterruptLineInit, KernelCfg1,
            KernelCfg2, Port, State, TaskAttr, TaskCb, TimeoutRef, TimerAttr,
            TimerCb, SemaphoreCb, MutexCb, PortThreading, readyqueue,
            arrayvec::ArrayVec,
            utils::{
                AlignedStorage, FixedPrioBitmap, Init, RawCell, UIntegerWithBound,
            },
        };

        type System = $crate::System<$Traits>;

        // Kernel-independent configuration process
        // ---------------------------------------------------------------------

        const fn build_cfg_phase1(
            allocator: &ConstAllocator,
        ) -> r3_core::kernel::cfg::CfgPhase1Data<System> {
            // Safety: We are `build!`, so it's okay to use `CfgBuilder::new`
            let mut my_cfg = unsafe { CfgBuilder::new(allocator) };
            r3_core::kernel::cfg::cfg_phase1!(
                let mut cfg = Cfg::<CfgBuilder<$Traits>>::new(&mut my_cfg, allocator));
            $configure(&mut cfg);
            CfgBuilder::finalize_in_cfg(&mut cfg);

            // Get `KernelStaticParams`, which is necessary for the later phases
            // of the finalization. Throw away `my_cfg` for now.
            cfg.finish_phase1()
        }

        // Implement `CfgPhase1` on `$Traits` using the information
        // collected in phase 1
        r3_core::kernel::cfg::attach_phase1!(
            ConstAllocator::with(build_cfg_phase1),
            impl CfgPhase1<System> for $Traits,
        );

        const fn build_cfg_phase2(
            allocator: &ConstAllocator,
        ) -> r3_core::kernel::cfg::CfgPhase2Data<System> {
            // Safety: We are `build!`, so it's okay to use `CfgBuilder::new`
            let mut my_cfg = unsafe { CfgBuilder::new(allocator) };
            r3_core::kernel::cfg::cfg_phase2!(
                let mut cfg = Cfg::<CfgBuilder<$Traits>>::new(&mut my_cfg, allocator));
            $configure(&mut cfg);
            CfgBuilder::finalize_in_cfg(&mut cfg);

            // Get `KernelStaticParams`, which is necessary for the later phases
            // of the finalization. Throw away `my_cfg` for now.
            cfg.finish_phase2()
        }

        // Implement `CfgPhase2` on `$Traits` using the information
        // collected in phase 2
        r3_core::kernel::cfg::attach_phase2!(
            ConstAllocator::with(build_cfg_phase2),
            impl CfgPhase2<System> for $Traits,
        );

        const fn build_cfg_phase3(
            allocator: &ConstAllocator,
        ) -> (
            MiddleCfg<$Traits>,
            $IdMap,
            r3_core::kernel::cfg::CfgPhase3Data<System>,
        ) {
            // Safety: We are `build!`, so it's okay to use `CfgBuilder::new`
            let mut my_cfg = unsafe { CfgBuilder::new(allocator) };
            r3_core::kernel::cfg::cfg_phase3!(
                let mut cfg = Cfg::<CfgBuilder<$Traits>>::new(&mut my_cfg, allocator));
            let id_map = $configure(&mut cfg);
            CfgBuilder::finalize_in_cfg(&mut cfg);

            // Do the finalization. This makes the final changes to
            // `my_cfg`
            cfg.finish_phase3_interrupt();
            let phase3_data = cfg.finish_phase3();

            (my_cfg.into_middle(), id_map, phase3_data)
        }

        const CFG_OUTPUT: (
            MiddleCfg<$Traits>,
            $IdMap,
            r3_core::kernel::cfg::CfgPhase3Data<System>,
        ) = ConstAllocator::with(build_cfg_phase3);
        const CFG: MiddleCfg<$Traits> = CFG_OUTPUT.0;

        // Implement `KernelStatic` on `$Traits` using the information
        // collected in phase 3
        r3_core::kernel::cfg::attach_phase3!(
            CFG_OUTPUT.2,
            impl KernelStatic<System> for $Traits,
        );

        // Kernel-specific configuration process
        // ---------------------------------------------------------------------

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
                (0..CFG.tasks.len()).map(|i| CFG.tasks[i].get().to_attr());
            static TASK_CB_POOL:
                [TaskCb<$Traits>; _] =
                    (0..CFG.tasks.len()).map(|i| CFG.tasks[i].get().to_state(&TASK_ATTR_POOL[i]));
        }

        // Instantiiate event group structures
        $crate::array_item_from_fn! {
            static EVENT_GROUP_CB_POOL:
                [EventGroupCb<$Traits>; _] =
                    (0..CFG.event_groups.len()).map(|i| CFG.event_groups[i].get().to_state());
        }

        // Instantiiate mutex structures
        $crate::array_item_from_fn! {
            static MUTEX_CB_POOL:
                [MutexCb<$Traits>; _] =
                    (0..CFG.mutexes.len()).map(|i| CFG.mutexes[i].get().to_state());
        }

        // Instantiiate semaphore structures
        $crate::array_item_from_fn! {
            static SEMAPHORE_CB_POOL:
                [SemaphoreCb<$Traits>; _] =
                    (0..CFG.semaphores.len()).map(|i| CFG.semaphores[i].get().to_state());
        }

        // Instantiiate timer structures
        $crate::array_item_from_fn! {
            const TIMER_ATTR_POOL: [TimerAttr<$Traits>; _] =
                (0..CFG.timers.len()).map(|i| CFG.timers[i].get().to_attr());
            static TIMER_CB_POOL:
                [TimerCb<$Traits>; _] =
                    (0..CFG.timers.len()).map(|i| CFG.timers[i].get().to_state(&TIMER_ATTR_POOL[i], i));
        }

        // Instantiate hunks
        static HUNK_POOL: RawCell<AlignedStorage<{ CFG.hunk_pool_len }, { CFG.hunk_pool_align }>> =
            Init::INIT;

        // Instantiate the global state
        type KernelState = State<$Traits>;
        static KERNEL_STATE: KernelState = State::INIT;

        // Construct a table of interrupt handlers
        const INTERRUPT_HANDLER_TABLE_LEN: usize =
            cfg::interrupt_handler_table_len(CFG.interrupt_lines);
        const INTERRUPT_HANDLER_TABLE:
            [Option<r3_core::kernel::interrupt::InterruptHandlerFn>; INTERRUPT_HANDLER_TABLE_LEN] =
            cfg::interrupt_handler_table(CFG.interrupt_lines);

        // Construct a table of interrupt line initiializers
        $crate::array_item_from_fn! {
            const INTERRUPT_LINE_INITS:
                [InterruptLineInit; _] =
                    (0..CFG.interrupt_lines.len()).map(|i| CFG.interrupt_lines[i].get().to_init());
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
                _phantom: Init::INIT,
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
    hunk_pool_len: usize,
    hunk_pool_align: usize,
    tasks: ComptimeVec<CfgBuilderTask<Traits>>,
    num_task_priority_levels: usize,
    interrupt_lines: ComptimeVec<CfgBuilderInterruptLine>,
    startup_hook: Option<fn()>,
    event_groups: ComptimeVec<CfgBuilderEventGroup>,
    mutexes: ComptimeVec<CfgBuilderMutex>,
    semaphores: ComptimeVec<CfgBuilderSemaphore>,
    timers: ComptimeVec<CfgBuilderTimer>,
}

/// The product of a [`CfgBuilder`]. [`build!`] will use it to define static
/// items and associate them with `Traits`.
///
/// This is not a real public interface, but needs to be `pub` so [`build!`] can
/// access the contents.
// FIXME: Not anymore
#[doc(hidden)]
pub struct MiddleCfg<Traits: KernelTraits> {
    pub hunk_pool_len: usize,
    pub hunk_pool_align: usize,
    pub tasks: &'static [Frozen<CfgBuilderTask<Traits>>],
    pub num_task_priority_levels: usize,
    pub interrupt_lines: &'static [Frozen<CfgBuilderInterruptLine>],
    pub startup_hook: Option<fn()>,
    pub event_groups: &'static [Frozen<CfgBuilderEventGroup>],
    pub mutexes: &'static [Frozen<CfgBuilderMutex>],
    pub semaphores: &'static [Frozen<CfgBuilderSemaphore>],
    pub timers: &'static [Frozen<CfgBuilderTimer>],
}

impl<Traits: KernelTraits> CfgBuilder<Traits> {
    /// Construct a `CfgBuilder`.
    ///
    /// # Safety
    ///
    /// This is only meant to be used by [`build!`]. Every instance of
    /// `CfgBuilder` destined for a particular kernel trait type and exposed to
    /// user code must be built through the same sequence of configuration
    /// operations. An instance of `CfgBuilder` violating this principle could
    /// be used to create object handles with arbitrary values to circumvent the
    /// compile-time access control of kernel objects.
    #[doc(hidden)]
    pub const unsafe fn new(allocator: &ConstAllocator) -> Self {
        Self {
            hunk_pool_len: 0,
            hunk_pool_align: 1,
            tasks: ComptimeVec::new_in(allocator.clone()),
            num_task_priority_levels: 4,
            interrupt_lines: ComptimeVec::new_in(allocator.clone()),
            startup_hook: None,
            event_groups: ComptimeVec::new_in(allocator.clone()),
            mutexes: ComptimeVec::new_in(allocator.clone()),
            semaphores: ComptimeVec::new_in(allocator.clone()),
            timers: ComptimeVec::new_in(allocator.clone()),
        }
    }

    /// Get `MiddleCfg`, consuming `self`.
    #[doc(hidden)]
    pub const fn into_middle(self) -> MiddleCfg<Traits> {
        MiddleCfg {
            hunk_pool_len: self.hunk_pool_len,
            hunk_pool_align: self.hunk_pool_align,
            tasks: Frozen::leak_slice(&self.tasks),
            num_task_priority_levels: self.num_task_priority_levels,
            interrupt_lines: Frozen::leak_slice(&self.interrupt_lines),
            startup_hook: self.startup_hook,
            event_groups: Frozen::leak_slice(&self.event_groups),
            mutexes: Frozen::leak_slice(&self.mutexes),
            semaphores: Frozen::leak_slice(&self.semaphores),
            timers: Frozen::leak_slice(&self.timers),
        }
    }

    /// Apply post-processing before [`r3_core::kernel::Cfg`] is finalized.
    #[doc(hidden)]
    pub const fn finalize_in_cfg(cfg: &mut r3_core::kernel::Cfg<Self>) {
        // Create hunks for task stacks.
        let mut i = 0;
        let mut tasks = &mut cfg.raw().tasks;
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
                tasks = &mut cfg.raw().tasks;

                tasks[i].stack = crate::task::StackHunk::from_hunk(hunk, size);
            }
            i += 1;
        }
    }
}

unsafe impl<Traits: KernelTraits> const r3_core::kernel::raw_cfg::CfgBase for CfgBuilder<Traits> {
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

        self.num_task_priority_levels = new_value;
    }

    fn startup_hook_define(&mut self, func: fn()) {
        assert!(
            self.startup_hook.is_none(),
            "only one combined startup hook can be registered"
        );
        self.startup_hook = Some(func);
    }
}
