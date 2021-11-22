//! Kernel configuration
use crate::{
    kernel::{hook, interrupt, raw, raw_cfg},
    utils::{ComptimeVec, Init, PhantomInvariant},
};

/// Wraps a [`raw_cfg::CfgBase`] to provide higher-level services.
pub struct Cfg<'c, C: raw_cfg::CfgBase> {
    raw: &'c mut C,
    finish_phase: u8,
    finish_interrupt_phase: u8,
    pub(super) startup_hooks: ComptimeVec<hook::CfgStartupHook>,
    pub(super) hunk_pool_len: usize,
    pub(super) hunk_pool_align: usize,
    pub(super) interrupt_lines: ComptimeVec<interrupt::CfgInterruptLineInfo>,
    pub(super) interrupt_handlers: ComptimeVec<interrupt::CfgInterruptHandler>,
}

impl<'c, C: raw_cfg::CfgBase> Cfg<'c, C> {
    /// Construct `Cfg`.
    pub const fn new(raw: &'c mut C) -> Self {
        Self {
            raw,
            finish_phase: 0,
            finish_interrupt_phase: 0,
            startup_hooks: ComptimeVec::new(),
            hunk_pool_len: 0,
            hunk_pool_align: 1,
            interrupt_lines: ComptimeVec::new(),
            interrupt_handlers: ComptimeVec::new(),
        }
    }

    /// Mutably borrow the underlying `C`.
    pub const fn raw(&mut self) -> &mut C {
        self.raw
    }

    /// Specify the number of task priority levels.
    ///
    /// The RAM consumption by task ready queues is expected to be proportional
    /// to the number of task priority levels. In addition, the scheduler may be
    /// heavily optimized for the cases where the number is very small (e.g., <
    /// `16`). This optimization can provide a significant performance
    /// improvement if the target processor does not have a CTZ (count trailing
    /// zero) instruction, barrel shifter, or hardware multiplier.
    ///
    /// Kernels may set an arbitrary upper bound for the number of task priority
    /// levels.
    pub const fn num_task_priority_levels(&mut self, new_value: usize)
    where
        // FIXME: `~const` is not allowed in `impl`??
        C: ~const raw_cfg::CfgBase,
    {
        self.raw.num_task_priority_levels(new_value);
    }

    /// Perform the first half of the finalization.
    ///
    /// This method makes the second last set of changes to the referenced `C:
    /// impl CfgBase`. It also constructs [`KernelStaticParams`], which must be
    /// passed to [`attach_static!`].
    pub const fn finish_pre(&mut self) -> KernelStaticParams<C::System> {
        assert!(
            self.finish_phase == 0,
            "finalization is already in progress (note: application code should \
            not  initiate the finalization!)"
        );
        self.finish_phase = 1;

        hook::sort_hooks(&mut self.startup_hooks);
        interrupt::sort_handlers(&mut self.interrupt_handlers);

        KernelStaticParams {
            _phantom: Init::INIT,
            startup_hooks: self.startup_hooks.map(hook::CfgStartupHook::to_attr),
            hunk_pool_len: self.hunk_pool_len,
            hunk_pool_align: self.hunk_pool_align,
            interrupt_handlers: self.interrupt_handlers,
        }
    }

    /// Perform additional finalization tasks for interrupt line configuration.
    ///
    /// This method must be called after `finish_pre` and before `finish_post`
    /// if `C` implements [`CfgInterruptLine`].
    ///
    /// [`CfgInterruptLine`]: raw_cfg::CfgInterruptLine
    pub const fn finish_interrupt(&mut self)
    where
        C: ~const raw_cfg::CfgInterruptLine,
        C::System: KernelStatic + raw::KernelInterruptLine,
    {
        assert!(
            self.finish_phase == 1,
            "pre-finalization (`Cfg::finish_pre`) isn't done yet on this `Cfg`"
        );
        assert!(
            self.finish_interrupt_phase == 0,
            "interrupt line finalization (`Cfg::finish_post_interrupt`) was \
            already done on this `Cfg`"
        );
        self.finish_interrupt_phase = 1;

        interrupt::panic_if_unmanaged_safety_is_violated::<C::System>(
            &self.interrupt_lines,
            &self.interrupt_handlers,
        );

        let mut i = 0;
        while i < self.interrupt_lines.len() {
            let interrupt_line = self.interrupt_lines.get(i);
            let start = C::System::INTERRUPT_HANDLERS[interrupt_line.num];
            self.raw.interrupt_line_define(
                raw_cfg::InterruptLineDescriptor {
                    phantom: Init::INIT,
                    line: interrupt_line.num,
                    priority: interrupt_line.priority,
                    start,
                    enabled: interrupt_line.enabled,
                },
                (),
            );
            i += 1;
        }

        // Clear these fields to indicate that this method has been called
        // as required
        self.interrupt_lines = ComptimeVec::new();
        self.interrupt_handlers = ComptimeVec::new();
    }

    /// Perform the second half of the finalization.
    ///
    /// This method makes the last set of changes to the referenced `C: impl
    /// CfgBase`.
    ///
    /// The finalization is divided as such so that `finish_post` can use the
    /// result of [`attach_static!`], which is derived from the product of
    /// [`finish_pre`].
    pub const fn finish_post(self)
    where
        C: ~const raw_cfg::CfgBase,
        C::System: KernelStatic,
    {
        assert!(
            self.finish_phase == 1,
            "pre-finalization (`Cfg::finish_pre`) isn't done yet on this `Cfg`"
        );

        assert!(
            self.interrupt_lines.is_empty() && self.interrupt_handlers.is_empty(),
            "missing call to `Cfg::finish_interrupt`"
        );

        // Register the combined startup hook
        self.raw.startup_hook_define(startup_hook::<C::System>);

        #[inline(always)]
        fn startup_hook<System: KernelStatic>() {
            for startup_hook in System::STARTUP_HOOKS.iter() {
                (startup_hook.start)(startup_hook.param);
            }
        }
    }
}

/// The inputs to [`attach_static!`].
///
/// The members of this trait are implementation details and not meant to be
/// used externally.
pub struct KernelStaticParams<System> {
    _phantom: PhantomInvariant<System>,
    pub startup_hooks: ComptimeVec<hook::StartupHookAttr>,
    pub hunk_pool_len: usize,
    pub hunk_pool_align: usize,
    pub interrupt_handlers: ComptimeVec<interrupt::CfgInterruptHandler>,
}

/// Associates static data to a system type.
///
/// The members of this trait are implementation details and not meant to be
/// implemented externally. Use [`attach_static!`] or [`DelegateKernelStatic`]
/// to implement this trait.
pub trait KernelStatic<System = Self> {
    const STARTUP_HOOKS: &'static [hook::StartupHookAttr];
    const INTERRUPT_HANDLERS: &'static [Option<interrupt::InterruptHandlerFn>];
    fn hunk_pool_ptr() -> *mut u8;
}

pub trait DelegateKernelStatic<System> {
    type Target: KernelStatic<System>;
}

impl<T: DelegateKernelStatic<System>, System> KernelStatic<System> for T {
    const STARTUP_HOOKS: &'static [hook::StartupHookAttr] = T::Target::STARTUP_HOOKS;
    const INTERRUPT_HANDLERS: &'static [Option<interrupt::InterruptHandlerFn>] =
        T::Target::INTERRUPT_HANDLERS;

    #[inline(always)]
    fn hunk_pool_ptr() -> *mut u8 {
        T::Target::hunk_pool_ptr()
    }
}

/// Implement [`KernelStatic`] on `$Ty` using the given `$params:
/// `[`KernelStaticParams`]`<$System>` to associate static data with the system
/// type `$System`.
///
/// This macro produces `static` items and a `KernelStatic<$System>`
/// implementation for `$Ty`. It doesn't support generics.
pub macro attach_static($params:expr, impl KernelStatic<$System:ty> for $Ty:ty $(,)?) {
    const _: () = {
        use $crate::{
            kernel::{cfg, hook, interrupt},
            utils::{for_times::U, AlignedStorage, Init, RawCell},
        };

        const STATIC_PARAMS: cfg::KernelStaticParams<$System> = $params;

        // Instantiate hunks
        static HUNK_POOL: RawCell<
            AlignedStorage<{ STATIC_PARAMS.hunk_pool_len }, { STATIC_PARAMS.hunk_pool_align }>,
        > = Init::INIT;

        // Construct a table of startup hooks
        array_item_from_fn! {
            const STARTUP_HOOKS: [hook::StartupHookAttr; _] =
                (0..STATIC_PARAMS.startup_hooks.len())
                    .map(|i| STATIC_PARAMS.startup_hooks.get(i));
        }

        // Consturct a table of combined second-level interrupt handlers
        const INTERRUPT_HANDLERS: [interrupt::CfgInterruptHandler; {
            STATIC_PARAMS.interrupt_handlers.len()
        }] = STATIC_PARAMS.interrupt_handlers.to_array();
        const NUM_INTERRUPT_HANDLERS: usize = INTERRUPT_HANDLERS.len();
        const NUM_INTERRUPT_LINES: usize =
            interrupt::num_required_interrupt_line_slots(&INTERRUPT_HANDLERS);
        struct Handlers;
        impl interrupt::CfgInterruptHandlerList for Handlers {
            type NumHandlers = U<NUM_INTERRUPT_HANDLERS>;
            const HANDLERS: &'static [Option<interrupt::InterruptHandlerFn>] = &INTERRUPT_HANDLERS;
        }
        const INTERRUPT_HANDLERS_SIZED: [Option<interrupt::InterruptHandlerFn>;
            NUM_INTERRUPT_LINES] = unsafe {
            // Safety: (1) We are `build!`, so it's okay to call this.
            //         (2) `INTERRUPT_HANDLERS` contains at least
            //             `NUM_INTERRUPT_HANDLERS` elements.
            interrupt::new_interrupt_handler_table::<
                $System,
                U<NUM_INTERRUPT_LINES>,
                Handlers,
                NUM_INTERRUPT_LINES,
                NUM_INTERRUPT_HANDLERS,
            >()
        };

        impl $crate::kernel::cfg::KernelStatic<$System> for $Ty {
            const STARTUP_HOOKS: &'static [hook::StartupHookAttr] = &STARTUP_HOOKS;

            const INTERRUPT_HANDLERS: &'static [Option<interrupt::InterruptHandlerFn>] =
                &INTERRUPT_HANDLERS;

            #[inline(always)]
            fn hunk_pool_ptr() -> *mut u8 {
                HUNK_POOL.get() as *mut u8
            }
        }
    };
}

macro array_item_from_fn($(
    $static_or_const:tt $out:ident: [$ty:ty; _] = (0..$len:expr).map(|$var:ident| $map:expr);
)*) {$(
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
)*}
