//! Kernel configuration
use crate::{
    kernel::{hook, interrupt, raw, raw_cfg},
    utils::{ComptimeVec, ConstAllocator, Frozen, Init, PhantomInvariant},
};

macro overview_ref() {
    "See [`KernelStatic`][]'s documentation for an overview of the
    configuration process and the traits involved."
}

/// Wraps a [`raw_cfg::CfgBase`] to provide higher-level services.
pub struct Cfg<'c, C: raw_cfg::CfgBase> {
    raw: &'c mut C,
    st: CfgSt,
    pub(super) startup_hooks: ComptimeVec<hook::CfgStartupHook>,
    pub(super) hunk_pool_len: usize,
    pub(super) hunk_pool_align: usize,
    pub(super) interrupt_lines: ComptimeVec<interrupt::CfgInterruptLineInfo>,
    pub(super) interrupt_handlers: ComptimeVec<interrupt::CfgInterruptHandler>,
}

#[derive(PartialEq, Eq)]
enum CfgSt {
    Phase1,
    Phase2,
    Phase3 { interrupts: bool },
}

impl<'c, C: raw_cfg::CfgBase> Cfg<'c, C> {
    /// Construct `Cfg`.
    const fn new(raw: &'c mut C, allocator: &'c ConstAllocator, st: CfgSt) -> Self {
        Self {
            raw,
            st,
            startup_hooks: ComptimeVec::new_in(allocator.clone()),
            hunk_pool_len: 0,
            hunk_pool_align: 1,
            interrupt_lines: ComptimeVec::new_in(allocator.clone()),
            interrupt_handlers: ComptimeVec::new_in(allocator.clone()),
        }
    }

    #[doc(hidden)]
    pub const fn __internal_new_phase1(
        raw: &'c mut C,
        allocator: &'c ConstAllocator,
        _dummy: &'c mut (),
    ) -> Self {
        Self::new(raw, allocator, CfgSt::Phase1)
    }

    #[doc(hidden)]
    pub const fn __internal_new_phase2(
        raw: &'c mut C,
        allocator: &'c ConstAllocator,
        _dummy: &'c mut (),
    ) -> Self
    where
        C::System: CfgPhase1,
    {
        Self::new(raw, allocator, CfgSt::Phase2)
    }

    #[doc(hidden)]
    pub const fn __internal_new_phase3(
        raw: &'c mut C,
        allocator: &'c ConstAllocator,
        _dummy: &'c mut (),
    ) -> Self
    where
        C::System: CfgPhase2,
    {
        Self::new(raw, allocator, CfgSt::Phase3 { interrupts: false })
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

    /// Finalize `self` for the phase 1 configuration.
    ///
    /// This method constructs [`CfgPhase1Data`], which must be passed to
    /// [`attach_phase1!`][] to proceed with [the kernel-independent
    /// configuration process][1].
    ///
    /// [1]: KernelStatic
    pub const fn finish_phase1(&mut self) -> CfgPhase1Data<C::System> {
        assert!(
            matches!(self.st, CfgSt::Phase1),
            "this `Cfg` wasn't made for phase 1 configuration"
        );

        hook::sort_hooks(&mut self.startup_hooks);
        interrupt::sort_handlers(&mut self.interrupt_handlers);

        CfgPhase1Data {
            _phantom: Init::INIT,
            startup_hooks: Frozen::leak_slice(
                &self.startup_hooks.map(hook::CfgStartupHook::to_attr),
            ),
            hunk_pool_len: self.hunk_pool_len,
            hunk_pool_align: self.hunk_pool_align,
            interrupt_handlers: Frozen::leak_slice(&self.interrupt_handlers),
        }
    }

    /// Finalize `self` for the phase 2 configuration.
    ///
    /// This method constructs [`CfgPhase2Data`], which must be passed to
    /// [`attach_phase2!`][] to proceed with [the kernel-independent
    /// configuration process][1].
    ///
    /// [1]: KernelStatic
    pub const fn finish_phase2(&mut self) -> CfgPhase2Data<C::System> {
        assert!(
            matches!(self.st, CfgSt::Phase2),
            "this `Cfg` wasn't made for phase 2 configuration"
        );

        hook::sort_hooks(&mut self.startup_hooks);
        interrupt::sort_handlers(&mut self.interrupt_handlers);

        CfgPhase2Data {
            _phantom: Init::INIT,
        }
    }

    /// Perform additional finalization tasks for interrupt line configuration.
    ///
    /// This method must be called before [`Self::finish_phase3`]
    /// if `C` implements [`CfgInterruptLine`].
    ///
    /// [`CfgInterruptLine`]: raw_cfg::CfgInterruptLine
    pub const fn finish_phase3_interrupt(&mut self)
    where
        C: ~const raw_cfg::CfgInterruptLine,
        C::System: CfgPhase2 + raw::KernelInterruptLine,
    {
        match &mut self.st {
            CfgSt::Phase3 { interrupts } => {
                assert!(
                    !*interrupts,
                    "interrupt line finalization (`Cfg::finish_phase3_interrupt`)
                    has already been done on this `Cfg`"
                );
                *interrupts = true;
            }
            _ => {
                panic!("this `Cfg` wasn't made for phase 3 configuration");
            }
        }

        interrupt::panic_if_unmanaged_safety_is_violated::<C::System>(
            &self.interrupt_lines,
            &self.interrupt_handlers,
        );

        let mut i = 0;
        while i < self.interrupt_lines.len() {
            let interrupt_line = &self.interrupt_lines[i];
            // FIXME: `<[T]>::get` is not `const fn` yet
            let start = if interrupt_line.num < C::System::CFG_INTERRUPT_HANDLERS.len() {
                C::System::CFG_INTERRUPT_HANDLERS[interrupt_line.num]
            } else {
                None
            };
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
        self.interrupt_lines.clear();
        self.interrupt_handlers.clear();
    }

    /// Finalize `self` for the phase 3 configuration.
    ///
    /// This method makes the last set of changes to the referenced `C: impl
    /// CfgBase`. It also constructs [`CfgPhase3Data`], which must be passed to
    /// [`attach_phase3!`][] to complete [the kernel-independent configuration
    /// process][1].
    ///
    /// [1]: KernelStatic
    pub const fn finish_phase3(self) -> CfgPhase3Data<C::System>
    where
        C: ~const raw_cfg::CfgBase,
        C::System: CfgPhase2,
    {
        assert!(
            matches!(self.st, CfgSt::Phase3 { .. }),
            "this `Cfg` wasn't made for phase 3 configuration"
        );

        assert!(
            self.interrupt_lines.is_empty() && self.interrupt_handlers.is_empty(),
            "missing call to `Cfg::finish_phase3_interrupt`"
        );

        // Register the combined startup hook
        self.raw.startup_hook_define(startup_hook::<C::System>);

        #[inline(always)]
        fn startup_hook<System: CfgPhase2>() {
            for startup_hook in System::CFG_STARTUP_HOOKS.iter() {
                (startup_hook.start)(startup_hook.param);
            }
        }

        CfgPhase3Data {
            _phantom: Init::INIT,
        }
    }
}

/// The inputs to [`attach_phase1!`].
///
/// The members of this trait are implementation details and not meant to be
/// used externally. They are nevertheless exposed for use by macro and for
/// transparency.
///
#[doc = overview_ref!()]
pub struct CfgPhase1Data<System> {
    _phantom: PhantomInvariant<System>,
    pub startup_hooks: &'static [Frozen<hook::StartupHookAttr>],
    pub hunk_pool_len: usize,
    pub hunk_pool_align: usize,
    pub interrupt_handlers: &'static [Frozen<interrupt::CfgInterruptHandler>],
}

/// The inputs to [`attach_phase2!`].
///
/// The members of this trait are implementation details and not meant to be
/// used externally. They are nevertheless exposed for use by macro and for
/// transparency.
///
#[doc = overview_ref!()]
pub struct CfgPhase2Data<System> {
    _phantom: PhantomInvariant<System>,
}

/// The inputs to [`attach_phase3!`].
///
/// The members of this trait are implementation details and not meant to be
/// used externally. They are nevertheless exposed for use by macro and for
/// transparency.
///
#[doc = overview_ref!()]
pub struct CfgPhase3Data<System> {
    _phantom: PhantomInvariant<System>,
}

/// Associates static data to a system type.
///
/// The members of this trait are implementation details and not meant to be
/// used externally. Use [`attach_phase3!`] or [`DelegateKernelStatic`]
/// to implement this trait.
///
/// # Derivation and Usage
///
/// This is one of the traits implemented by **the kernel-independent
/// configuration process**. This process is divided into three phases. The
/// first phase involves the following steps, mostly happening in a
/// macro-generated function (let's call this `do_cfg_phase1`):
///
///  - `do_cfg_phase1` constructs a `C: const `[`CfgBase`][] and binds it to a
///    local variable `c`.
///  - `do_cfg_phase1` invokes [`cfg_phase1!`][], passing `&mut c`, to construct
///    and bind a [`Cfg`][]`<C>` to a local variable `b`.
///  - `do_cfg_phase1` invokes an application-provided configuration function to
///    register objects through `b`.
///  - `do_cfg_phase1` calls [`Cfg::finish_phase1`][] to obtain a
///    [`CfgPhase1Data`][].
///  - `do_cfg_phase1` returns this `CfgPhase1Data`.
///  - Using this `CfgPhase1Data`, [`attach_phase1!`][] produces `static` items
///    and an implementation of [`CfgPhase1`][] for `C::System` (directly or
///    indirectly through [`DelegateKernelStatic`]).
///
/// [`CfgBase`]: raw_cfg::CfgBase
///
/// The remaining phases repeat these steps using the prospective macros and
/// functions as well as the constant values derived so far (through the
/// implemented traits), each time recreating `C` and `Cfg<C>` from scratch.
/// The final phase produces the finalized `C`, which the kernel-specific
/// configuration process can use to complete the rest of the configuration
/// process. (`C`s produced by other phases are incomplete and therefore
/// should be disregarded.) The final phase also produces an implementation of
/// `KernelStatic` for `C::System`.
///
/// <div class="admonition-follows"></div>
///
/// > **Rationale:**
/// > Usually `const fn`s can't use constant values derived by themselves as
/// > constant values, but splitting into multiple phases makes this possible.
/// >
/// > The current implementation doesn't fully utilize all of the three phases.
/// > The extra phases are kept to leave room for future internal changes.
///
/// The following diagram outlines the data flow in this process.
///
/// <center>
///
#[doc = svgbobdoc::transform!(
/// ```svgbob
///                        Kernel-provided            |     Application-provided
///                      configuration macro          |    configuration function
///         ---------------------------------------------------------------------------
///
///                   C                    "Cfg<C>"              "&mut Cfg<C>"
///
///                   │                       │                       │
///                  ┌┴┐C::new                │                       │
///                  │ │     "cfg_phase1!"    │                       │
///                  │ │ ------------------> ┌┴┐     $configure       │
///                  │ │                     │ │ ------------------> ┌┴┐
///                  │ │       finish_phase1 │ │                     └┬┘
///                  └┬┘          ,--------- └┬┘                      │
///                   │           |           │                       │
///                   │           v           │                       │
///                   │  .-----------------.  │                       │
///                   │  |  CfgPhase1Data  |  │                       │
///                   │  '-----------------'  │                       │
///                   │           |           │                       │
///                   │           v           │                       │
///                   │    "attach_phase1!"   │                       │
///                   │           |           │                       │
///                   │           v           │                       │
///                   │ .-------------------. │                       │
///                   │ |  impl CfgPhase1   | │                       │
///                   │ |   for C::System   | │                       │
///                   │ '-------------------' │                       │
///                   │        |              │                       │
///                  ┌┴┐C::new |              │                       │
///                  │ │       v "cfg_phase2!"│                       │
///                  │ │ ------+-----------> ┌┴┐     $configure       │
///                  │ │                     │ │ ------------------> ┌┴┐
///                  │ │       finish_phase2 │ │                     └┬┘
///                  └┬┘          .--------- └┬┘                      │
///                   │           |           │                       │
///                   │           v           │                       │
///                   │  .-----------------.  │                       │
///                   │  |  CfgPhase2Data  |  │                       │
///                   │  '-----------------'  │                       │
///                   │           |           │                       │
///                   │           v           │                       │
///                   │    "attach_phase2!"   │                       │
///                   │           |           │                       │
///                   │           v           │                       │
///                   │ .-------------------. │                       │
///                   │ |  impl CfgPhase2   | │                       │
///                   │ |   for C::System   | │                       │
///                   │ '-------------------' │                       │
///                   │        |              │                       │
///                  ┌┴┐C::new |              │                       │
///                  │ │       v "cfg_phase3!"│                       │
///                  │ │ ------+-----------> ┌┴┐     $configure       │
///                  │ │                     │ │ ------------------> ┌┴┐
///                  │ │    finish_phase3    │ │                     └┬┘
///                  │ │ <--------+--------- └┬┘                      │
///          .------ └┬┘          |           │                       │
///          |        │           v           │                       │
///          v        │  .-----------------.  │                       │
///   Kernel-specific │  |  CfgPhase3Data  |  │                       │
///    configuration  │  '-----------------'  │                       │
///       process     │           |           │                       │
///                   │           v           │                       │
///                   │    "attach_phase3!"   │                       │
///                   │           |           │                       │
///                   │           v           │                       │
///                   │ .-------------------. │                       │
///                   │ | impl KernelStatic | │                       │
///                   │ |   for C::System   | │                       │
///                   │ '-------------------' │                       │
///                   │                       │                       │
/// ```
)]
///
/// </center>
///
#[doc = include_str!("../common.md")]
pub trait KernelStatic<System = Self>: CfgPhase2<System> {}

/// The second precursor to [`KernelStatic`][].
///
/// The members of this trait are implementation details and not meant to be
/// used externally. Use [`attach_phase2!`] or [`DelegateKernelStatic`]
/// to implement this trait.
///
#[doc = overview_ref!()]
pub trait CfgPhase2<System = Self>: CfgPhase1<System> {}

/// The first precursor to [`KernelStatic`][].
///
/// The members of this trait are implementation details and not meant to be
/// used externally. Use [`attach_phase1!`] or [`DelegateKernelStatic`]
/// to implement this trait.
///
#[doc = overview_ref!()]
pub trait CfgPhase1<System = Self> {
    const CFG_STARTUP_HOOKS: &'static [hook::StartupHookAttr];
    const CFG_INTERRUPT_HANDLERS: &'static [Option<interrupt::InterruptHandlerFn>];
    fn cfg_hunk_pool_ptr() -> *mut u8;
}

/// The marker trait to generate a forwarding implementation of
/// [`KernelStatic`][]`<System>` as well as [`CfgPhase1`][]`<System>` and
/// [`CfgPhase2`][]`<System>`.
///
/// This is useful for circumventing [the orphan rules][1]. Suppose we have a
/// kernel crate `r3_kernel` and an application crate `app`, and `r3_kernel`
/// provides a system type `System<Traits>`, where `Traits` is a marker type to
/// be defined in an application crate. For many reasons, `static` items to
/// store a kernel state can only be defined in `app`, where the concrete form
/// of the kernel is known. This means `impl KernelStatic for System<Traits>`
/// has to appear in `app`, but since both `KernelStatic` and `System` are
/// foreign to `app`, this is not allowed by the orphan rules.
///
/// ```rust,ignore
/// // r3::kernel::cfg
/// // ========================
/// trait KernelStatic<System> {}
///
/// // r3_kernel
/// // ========================
/// struct System<Traits> { /* ... */ }
///
/// // app
/// // ========================
/// struct Traits;
/// impl r3::kernel::cfg::KernelStatic<r3_kernel::System<Traits>>
///     for r3_kernel::System<Traits> {} // E0117
/// ```
///
/// The above example can be fixed by implementing `KernelStatic` on `Traits`
/// instead and `DelegateKernelStatic` on `System`.
///
/// ```rust,ignore
/// // r3::kernel::cfg
/// // ========================
/// trait KernelStatic<System> {}
/// trait DelegateKernelStatic<System> { type Target; }
/// impl<T, System> KernelStatic<System> for T
///     where T: DelegateKernelStatic<System> {}
///
/// // r3_kernel
/// // ========================
/// struct System<Traits> { /* ... */ }
/// impl<Traits> DelegateKernelStatic for System<Traits> {
///     // Inherit `Traits`'s implementation
///     type Target = Traits;
/// }
///
/// // app
/// // ========================
/// struct Traits;
/// impl r3::kernel::cfg::KernelStatic<r3_kernel::System<Traits>>
///     for Traits {} // OK
/// ```
///
/// [1]: https://rust-lang.github.io/rfcs/2451-re-rebalancing-coherence.html#concrete-orphan-rules
pub trait DelegateKernelStatic<System> {
    type Target;
}

impl<T: DelegateKernelStatic<System>, System> KernelStatic<System> for T where
    T::Target: KernelStatic<System>
{
}

impl<T: DelegateKernelStatic<System>, System> CfgPhase2<System> for T where
    T::Target: CfgPhase2<System>
{
}

impl<T: DelegateKernelStatic<System>, System> CfgPhase1<System> for T
where
    T::Target: CfgPhase1<System>,
{
    const CFG_STARTUP_HOOKS: &'static [hook::StartupHookAttr] = T::Target::CFG_STARTUP_HOOKS;
    const CFG_INTERRUPT_HANDLERS: &'static [Option<interrupt::InterruptHandlerFn>] =
        T::Target::CFG_INTERRUPT_HANDLERS;

    #[inline(always)]
    fn cfg_hunk_pool_ptr() -> *mut u8 {
        T::Target::cfg_hunk_pool_ptr()
    }
}

/// Construct [`Cfg`]`<$RawCfg>` for the phase 3 configuration.
///
///  - `$raw_cfg: &mut impl `[`CfgBase`][]
///  - `$allocator: &`[`ConstAllocator`][]
///
/// `<$RawCfg as `[`CfgBase`][]`>::System` must implement [`CfgPhase2`][].
///
/// [`CfgBase`]: raw_cfg::CfgBase
pub macro cfg_phase3(
    let mut $cfg:ident = Cfg::<$RawCfg:ty>::new($raw_cfg:expr, $allocator:expr)
) {
    let mut dummy = ();
    let mut $cfg = Cfg::<$RawCfg>::__internal_new_phase3(&mut *$raw_cfg, $allocator, &mut dummy);
}

/// Construct [`Cfg`]`<$RawCfg>` for the phase 2 configuration.
///
///  - `$raw_cfg: &mut impl `[`CfgBase`][]
///  - `$allocator: &`[`ConstAllocator`][]
///
/// `<$RawCfg as `[`CfgBase`][]`>::System` must implement [`CfgPhase1`][].
///
/// [`CfgBase`]: raw_cfg::CfgBase
pub macro cfg_phase2(
    let mut $cfg:ident = Cfg::<$RawCfg:ty>::new($raw_cfg:expr, $allocator:expr)
) {
    let mut dummy = ();
    let mut $cfg = Cfg::<$RawCfg>::__internal_new_phase2(&mut *$raw_cfg, $allocator, &mut dummy);
}

/// Construct [`Cfg`]`<$RawCfg>` for the phase 1 configuration.
///
///  - `$raw_cfg: &mut impl `[`CfgBase`][]
///  - `$allocator: &`[`ConstAllocator`][]
///
/// [`CfgBase`]: raw_cfg::CfgBase
pub macro cfg_phase1(
    let mut $cfg:ident = Cfg::<$RawCfg:ty>::new($raw_cfg:expr, $allocator:expr)
) {
    let mut dummy = ();
    let mut $cfg = Cfg::<$RawCfg>::__internal_new_phase1(&mut *$raw_cfg, $allocator, &mut dummy);
}

/// Implement [`KernelStatic`] on `$Ty` using the given `$params:
/// `[`CfgPhase3Data`]`<$System>` to associate static data with the system
/// type `$System`.
///
/// This macro produces `static` items and a `KernelStatic<$System>`
/// implementation for `$Ty`. It doesn't support generics, which means this
/// macro should be invoked in an application crate, where the concrete system
/// type is known.
///
#[doc = overview_ref!()]
pub macro attach_phase3($params:expr, impl KernelStatic<$System:ty> for $Ty:ty $(,)?) {
    const _: () = {
        const _: $crate::kernel::cfg::CfgPhase3Data<$System> = $params;
        impl $crate::kernel::cfg::KernelStatic<$System> for $Ty {}
    };
}

/// Implement [`CfgPhase2`] on `$Ty` using the given `$params:
/// `[`CfgPhase2Data`]`<$System>` to associate static data with the system
/// type `$System`.
///
/// This macro produces `static` items and a `CfgPhase2<$System>`
/// implementation for `$Ty`. It doesn't support generics, which means this
/// macro should be invoked in an application crate, where the concrete system
/// type is known.
///
#[doc = overview_ref!()]
pub macro attach_phase2($params:expr, impl CfgPhase2<$System:ty> for $Ty:ty $(,)?) {
    const _: () = {
        const _: $crate::kernel::cfg::CfgPhase2Data<$System> = $params;
        impl $crate::kernel::cfg::CfgPhase2<$System> for $Ty {}
    };
}

/// Implement [`CfgPhase1`] on `$Ty` using the given `$params:
/// `[`CfgPhase1Data`]`<$System>` to associate static data with the system
/// type `$System`.
///
/// This macro produces `static` items and a `CfgPhase1<$System>`
/// implementation for `$Ty`. It doesn't support generics, which means this
/// macro should be invoked in an application crate, where the concrete system
/// type is known.
///
#[doc = overview_ref!()]
pub macro attach_phase1($params:expr, impl CfgPhase1<$System:ty> for $Ty:ty $(,)?) {
    const _: () = {
        use $crate::{
            kernel::{cfg, hook, interrupt},
            utils::{for_times::U, AlignedStorage, Init, RawCell},
        };

        const STATIC_PARAMS: cfg::CfgPhase1Data<$System> = $params;

        // Instantiate hunks
        static HUNK_POOL: RawCell<
            AlignedStorage<{ STATIC_PARAMS.hunk_pool_len }, { STATIC_PARAMS.hunk_pool_align }>,
        > = Init::INIT;

        // Construct a table of startup hooks
        array_item_from_fn! {
            const STARTUP_HOOKS: [hook::StartupHookAttr; _] =
                (0..STATIC_PARAMS.startup_hooks.len())
                    .map(|i| STATIC_PARAMS.startup_hooks[i].get());
        }

        // Consturct a table of combined second-level interrupt handlers
        array_item_from_fn! {
            const INTERRUPT_HANDLERS: [interrupt::CfgInterruptHandler; _] =
                (0..STATIC_PARAMS.interrupt_handlers.len())
                    .map(|i| STATIC_PARAMS.interrupt_handlers[i].get());
        }
        const NUM_INTERRUPT_HANDLERS: usize = INTERRUPT_HANDLERS.len();
        const NUM_INTERRUPT_LINES: usize =
            interrupt::num_required_interrupt_line_slots(&INTERRUPT_HANDLERS);
        struct Handlers;
        impl interrupt::CfgInterruptHandlerList for Handlers {
            type NumHandlers = U<NUM_INTERRUPT_HANDLERS>;
            const HANDLERS: &'static [interrupt::CfgInterruptHandler] = &INTERRUPT_HANDLERS;
        }
        const INTERRUPT_HANDLERS_COMBINED: [Option<interrupt::InterruptHandlerFn>;
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

        impl $crate::kernel::cfg::CfgPhase1<$System> for $Ty {
            const CFG_STARTUP_HOOKS: &'static [hook::StartupHookAttr] = &STARTUP_HOOKS;

            const CFG_INTERRUPT_HANDLERS: &'static [Option<interrupt::InterruptHandlerFn>] =
                &INTERRUPT_HANDLERS_COMBINED;

            #[inline(always)]
            fn cfg_hunk_pool_ptr() -> *mut u8 {
                HUNK_POOL.get() as *mut u8
            }
        }
    };
}

// FIXME: A false `unused_macros`; it's actually used by `attach_*!`
#[allow(unused_macros)]
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
