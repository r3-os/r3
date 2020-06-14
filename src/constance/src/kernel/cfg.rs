//! Static configuration mechanism for the kernel
use core::marker::PhantomData;

use crate::utils::{ComptimeVec, FIXED_PRIO_BITMAP_MAX_LEN};

mod event_group;
mod hunk;
mod task;
pub use self::{event_group::*, hunk::*, task::*};

/// Makes some useful macros available inside a configuration function.
///
/// # Examples
///
/// ```
/// #![feature(const_fn)]
/// use constance::kernel::{EventGroup, Kernel, Task};
///
/// constance::configure! {
///     const fn configure_app<System: Kernel>(_: &mut CfgBuilder<System>)
///         -> (Task<System>, EventGroup<System>)
///     {
///         set!(num_task_priority_levels = 4);
///         let task = build! { Task<_>,
///             start = task_body, priority = 3, active = true };
///         let eg = build! { EventGroup<_> };
///         (task, eg)
///     }
/// }
///
/// fn task_body(_: usize) {}
/// ```
///
/// The above code is equivalent to:
///
/// ```
/// #![feature(const_fn)]
/// use constance::kernel::{CfgBuilder, EventGroup, Kernel, Task};
///
/// const fn configure_app<System: Kernel>(b: &mut CfgBuilder<System>)
///     -> (Task<System>, EventGroup<System>)
/// {
///     b.num_task_priority_levels(4);
///     let task = Task::build()
///         .start(task_body).priority(3).active(true).finish(b);
///     let eg = EventGroup::build().finish(b);
///     (task, eg)
/// }
///
/// fn task_body(_: usize) {}
/// ```
///
/// # Available Macros
///
/// The following macros are available inside the function:
///
/// ## `set!(prop = value)`
///
/// Set a global propertry.
///
///  - `num_task_priority_levels = NUM_LEVELS: usize` specifies the number of
///    task priority levels. The default value is `16`.
///
/// ## `call!(expr, arg1, arg2, ...)`
///
/// Invokes another configuration function `expr`.
///
/// ## `build!(path, name1 = arg1, name2 = arg2, ...)`
///
/// Invokes a builder method `path::build`, calls modifying methods
/// `name1, name2, ...` on the builder, and then finally calls `finish`, which
/// is assumed to be a nullary configuration function.
///
/// Most kernel objects can be created in this way.
///
///  - [`Task`] with options defined in [`CfgTaskBuilder`]
///  - [`EventGroup`] with options defined in [`CfgEventGroupBuilder`]
///
/// [`Task`]: crate::kernel::Task
/// [`CfgTaskBuilder`]: crate::kernel::CfgTaskBuilder
/// [`EventGroup`]: crate::kernel::EventGroup
/// [`CfgEventGroupBuilder`]: crate::kernel::CfgEventGroupBuilder
///
/// ## `new_hunk!(T)`
///
/// Defines a new hunk. `T` must implement [`Init`](crate::utils::Init).
///
/// ## `new_hunk!([T], zeroed = true, len = LEN, align = ALIGN)`
///
/// Defines a new zero-initialized hunk of an array of the specified length and
/// alignment.
///
/// # Limitations
///
/// Generic parameters are supported with a help of
/// [`::parse_generics_shim`]. Not all forms of generics are supported. See its
/// documentation to find out what limitation applies.
///
/// `self` parameters aren't supported yet.
///
#[macro_export]
macro_rules! configure {
    (
        // (1) Top-level rule - Parse everything before generic parmameters.
        //     Pass the generic parameters to `parse_generics_shim!` and proceed
        //     to (2-1) or (2-2) with the result of `parse_generics_shim!`.
        $( #[$meta:meta] )*
        $vis:vis const fn $ident:ident $($gen_tokens:tt)*
    ) => {
        $crate::parse_generics_shim::parse_generics_shim! {
            { constr },
            then $crate::configure! {
                [2]
                meta: $( #[$meta] )*,
                vis: $vis,
                ident: $ident,
            },
            $($gen_tokens)*
        }
    };

    (
        // (2-1) Parse everything between generic parameters and an `where` clause.
        //       Pass the `where` clause to `parse_where_shim!` and proceed to (3)
        //       with the result of `parse_where_shim!`.
        [2]
        // Added by (1)
        meta: $( #[$meta:meta] )*,
        vis: $vis:vis,
        ident: $ident:ident,

        // Generated by `parse_generics_shim`
        $gen_param:tt,

        // Remaining tokens to parse
        (_: CfgBuilder<$sys:ty>) -> $id_map:ty where $($where_tokens:tt)*
    ) => {
        $crate::parse_generics_shim::parse_where_shim! {
            { clause, preds },
            then $crate::configure! {
                [3]
                meta: $( #[$meta] )*,
                vis: $vis,
                ident: $ident,
                sys: $sys,
                id_map: $id_map,
                gen_param: $gen_param,
            },
            where $($where_tokens)*
        }
    };

    (
        // (2-2) Same as (2-1) except `where` is absent.
        [2]
        // Added by (1)
        meta: $( #[$meta:meta] )*,
        vis: $vis:vis,
        ident: $ident:ident,

        // Generated by `parse_generics_shim`
        $gen_param:tt,

        // Remaining tokens to parse
        (_: &mut CfgBuilder<$sys:ty>) -> $id_map:ty { $($body:tt)* }
    ) => {
        $crate::parse_generics_shim::parse_where_shim! {
            { clause, preds },
            then $crate::configure! {
                [3]
                meta: $( #[$meta] )*,
                vis: $vis,
                ident: $ident,
                sys: $sys,
                id_map: $id_map,
                gen_param: $gen_param,
            },
            { $($body)* }
        }
    };

    (
        // (3) Parse everything after an optional `where`. Proceed to (4).
        [3]
        // Added by (1)
        meta: $( #[$meta:meta] )*,
        vis: $vis:vis,
        ident: $ident:ident,

        // Added by (2)
        sys: $sys:ty,
        id_map: $id_map:ty,
        gen_param: $gen_param:tt,

        // Generated by `parse_where_shim`
        $where_param:tt,

        // Remaining tokens to parse
        {
            $($tt:tt)*
        }
    ) => {
        $crate::configure! {
            dollar: [$],
            meta: $( #[$meta] )*,
            vis: $vis,
            ident: $ident,
            sys: $sys,
            id_map: $id_map,
            gen_param: $gen_param,
            where_param: $where_param,

            body: { $($tt)* },
        }
    };

    (
        // (4) Core rule - this is invoked by the top-level rule down below

        // This parameter (`dollar`) is used to produce a dollar token (`$`).
        //
        // When you write something like `$(  )*` in a macro output, the macro
        // transcriber interprets it as a repetition. This conflict with our
        // intent to generate `macro_rules!` because we don't want `$(...)*`
        // inside these generated `macro_rules!` to be processed. We need the
        // expansion of those `$(...)*` to happen when expanding the generated
        // `macro_rules!`, not when expanding `configure!`.
        //
        // We address this problem by receiving a dollar token via a
        // metavariable. The transcriber for `configure!` doesn't interpret the
        // contents of `$dollar` and simply copies them verbadim to the output
        // token stream, so we can use it anywhere in the macro output without
        // worrying about it being processed by the transcriber in an unintended
        // way.
        dollar: [$dollar:tt],
        meta: $( #[$meta:meta] )*,
        vis: $vis:vis,
        ident: $ident:ident,
        sys: $sys:ty,
        id_map: $id_map:ty,
        gen_param: {
            constr: [ $($gen_param_constr:tt)* ],
        },
        where_param: {
            clause: [ $($where_param_clause:tt)* ],
            preds: [ $($where_param_preds:tt)* ],
        },
        body: { $($tt:tt)* },
    ) => {
        $( #[$meta] )*
        #[allow(unused_macros)]
        $vis const fn $ident<$($gen_param_constr)*>(
            cfg: &mut $crate::kernel::CfgBuilder<$sys>
        ) -> $id_map
            $($where_param_clause)*
        {
            macro_rules! set {
                ($argname:ident = $arg:expr $dollar(,)*) => {{
                    cfg.$argname($arg);
                }};
            }

            macro_rules! call {
                ($path:expr $dollar(, $arg:expr)* $dollar(,)*) => {{
                    $path(cfg, $dollar($arg),*)
                }};
            }

            macro_rules! build {
                ($path:ty $dollar(, $argname:ident = $arg:expr)* $dollar(,)*) => {{
                    <$path>::build()
                        $dollar(. $argname($arg))*
                        .finish(cfg)
                }};
            }

            macro_rules! new_hunk {
                ([u8] $dollar(, zeroed = true)?, len = $len:expr) => {
                    new_hunk!([u8], zeroed = true, len = $len, align = 1)
                };
                ([$ty:ty], zeroed = true, len = $len:expr, align = $align:expr) => {
                    call!($crate::kernel::cfg_new_hunk_zero_array, $len, $align)
                };
                ($ty:ty) => {call!($crate::kernel::cfg_new_hunk::<_, $ty>)};
            }

            $($tt)*
        }
    };
}

/// Attach a configuration function (defined by [`configure!`]) to a "system"
/// type by implementing [`KernelCfg2`] on `$sys`.
///
/// [`KernelCfg2`]: crate::kernel::KernelCfg2
#[macro_export]
macro_rules! build {
    ($sys:ty, $configure:expr) => {{
        use $crate::{
            kernel::{
                CfgBuilder, CfgBuilderInner, EventGroupCb, HunkAttr, HunkInitAttr, KernelCfg1,
                KernelCfg2, Port, State, TaskAttr, TaskCb,
            },
            utils::{
                intrusive_list::StaticListHead, AlignedStorage, FixedPrioBitmap, Init, RawCell,
                UIntegerWithBound,
            },
        };

        // `$configure` produces two values: a `CfgBuilder` and an ID map
        // (custom type). We need the first one to be `const` so that we can
        // calculate the values of generic parameters based on its contents.
        const CFG: CfgBuilderInner<$sys> = {
            // Safety: We are `build!`, so it's okay to use `CfgBuilder::new`
            let mut cfg = unsafe { CfgBuilder::new() };
            $configure(&mut cfg);
            cfg.into_inner()
        };

        // The second value can be just `let`
        // Safety: We are `build!`, so it's okay to use `CfgBuilder::new`
        let id_map = $configure(&mut unsafe { CfgBuilder::new() });

        // Set up task priority levels
        type TaskPriority = UIntegerWithBound<{ CFG.num_task_priority_levels as u128 - 1 }>;
        $crate::array_item_from_fn! {
            const TASK_PRIORITY_LEVELS: [TaskPriority; _] =
                (0..CFG.num_task_priority_levels).map(|i| i as _);
        };

        // Safety: We are `build!`, so it's okay to `impl` this
        unsafe impl KernelCfg1 for $sys {
            const NUM_TASK_PRIORITY_LEVELS: usize = CFG.num_task_priority_levels;
            type TaskPriority = TaskPriority;
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

        // Instantiate hunks
        static HUNK_POOL: RawCell<AlignedStorage<{ CFG.hunk_pool_len }, { CFG.hunk_pool_align }>> =
            Init::INIT;
        const HUNK_INITS: [HunkInitAttr; { CFG.hunks.len() }] = CFG.hunks.to_array();

        // Task ready bitmap
        type TaskReadyBitmap = FixedPrioBitmap<{ CFG.num_task_priority_levels }>;

        // Instantiate the global state
        type KernelState = State<$sys>;
        static KERNEL_STATE: KernelState = State::INIT;

        // Safety: We are `build!`, so it's okay to `impl` this
        unsafe impl KernelCfg2 for $sys {
            type TaskReadyBitmap = TaskReadyBitmap;
            type TaskReadyQueue = [StaticListHead<TaskCb<Self>>; CFG.num_task_priority_levels];

            fn state() -> &'static KernelState {
                &KERNEL_STATE
            }

            const HUNK_ATTR: HunkAttr = HunkAttr {
                hunk_pool: || HUNK_POOL.get() as *const u8,
                inits: &HUNK_INITS,
            };

            #[inline(always)]
            fn task_cb_pool() -> &'static [TaskCb<$sys>] {
                &TASK_CB_POOL
            }

            #[inline(always)]
            fn event_group_cb_pool() -> &'static [EventGroupCb<$sys>] {
                &EVENT_GROUP_CB_POOL
            }
        }

        id_map
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! array_item_from_fn {
    ($(
        $static_or_const:tt $out:ident: [$ty:ty; _] = (0..$len:expr).map(|$var:ident| $map:expr);
    )*) => {$(
        $static_or_const $out: [$ty; { $len }] = {
            let mut values = [$crate::prelude::Init::INIT; { $len }];
            let mut i = 0;
            while i < $len {
                values[i] = {
                    let $var = i;
                    $map
                };
                i += 1;
            }
            values
        };
    )*};
}

/// A kernel configuration being constructed.
#[doc(hidden)]
pub struct CfgBuilder<System> {
    /// Disallows the mutation of `CfgBuilderInner` by a user-defined
    /// configuration function by making this not `pub`.
    inner: CfgBuilderInner<System>,
}

/// The private portion of [`CfgBuilder`]. This is not a real public interface,
/// but needs to be `pub` so [`build!`] can access the contents.
#[doc(hidden)]
pub struct CfgBuilderInner<System> {
    _phantom: PhantomData<System>,
    pub hunks: ComptimeVec<super::HunkInitAttr>,
    pub hunk_pool_len: usize,
    pub hunk_pool_align: usize,
    pub tasks: ComptimeVec<CfgBuilderTask<System>>,
    pub num_task_priority_levels: usize,
    pub event_groups: ComptimeVec<CfgBuilderEventGroup>,
}

impl<System> CfgBuilder<System> {
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
                hunks: ComptimeVec::new(),
                hunk_pool_len: 0,
                hunk_pool_align: 1,
                tasks: ComptimeVec::new(),
                num_task_priority_levels: 16,
                event_groups: ComptimeVec::new(),
            },
        }
    }

    /// Get `CfgBuilderInner`, consuming `self`.
    #[doc(hidden)]
    pub const fn into_inner(self) -> CfgBuilderInner<System> {
        self.inner
    }

    pub const fn num_task_priority_levels(&mut self, new_value: usize) {
        if new_value == 0 {
            panic!("`num_task_priority_levels` must be greater than zero");
        } else if new_value > FIXED_PRIO_BITMAP_MAX_LEN {
            panic!("`num_task_priority_levels` must be less than or equal to `FIXED_PRIO_BITMAP_MAX_LEN`");
        } else if new_value >= isize::max_value() as usize {
            // Limiting priority values in range `0..(isize::max_value() - 1)`
            // leaves room for special values outside the extremities.
            //
            // This branch is actually unreachable because
            // `FIXED_PRIO_BITMAP_MAX_LEN` is so small compared to the size of
            // `isize`.
            unreachable!();
        }

        self.inner.num_task_priority_levels = new_value;
    }
}
