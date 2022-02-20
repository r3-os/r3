#![doc = include_str!("../common.md")]
#![doc = r3_core::bind::__internal_module_doc!("r3_core", r#"
<div class="admonition-follows"></div>

> This module re-exports stable items from [`r3_core::bind`][] as well as
> providing some additional items.
"#)]
//!
//! # Examples
//!
#![doc = crate::tests::doc_test!(
/// ```rust
/// use r3::{
///     kernel::{StaticTask, StaticTimer},
///     bind::{bind, Bind},
///     time::Duration,
///     prelude::*,
/// };
///
/// # type Objects = ();
/// const fn configure_app<C>(cfg: &mut Cfg<C>)
/// where
///     C: ~const traits::CfgBase<System = System> +
///        ~const traits::CfgTask +
///        ~const traits::CfgTimer,
/// {
///     // Create a binding and give the timer an exclusive access
///     let count = bind((), || 0).finish(cfg);
///     StaticTimer::define()
///         // `(x,)` is a one-element tuple
///         .start_with_bind((count.borrow_mut(),), |count: &mut i32| {
///             // The counter persists across invocations
///             assert!(*count >= 5);
///             *count += 1;
/// #           if *count == 10 { exit(0); }
///         })
///         .period(Duration::from_millis(50))
///         .delay(Duration::ZERO)
///         .active(true)
///         .finish(cfg);
///
///     // Although we gave the timer an exclusive access to `count`,
///     // We can still borrow `count` temporarily before the timer
///     // starts running. (The initialization of bindings happens in
///     // startup hooks, which run before all tasks and timers.)
///     bind(
///         (count.borrow_mut(),),
///         |count: &mut i32| {
///             *count = 5;
///         },
///     ).unpure().finish(cfg);
///
///     // Create a binding
///     let num = bind((), || 42).finish(cfg);
///
///     // Alternatively, without using `bind`:
///     // let num = Bind::define().init(|| 42).finish(cfg);
///
///     // Then create a reference to it, a reference to the reference,
///     // and so on.
///     let num = bind((num.take_mut(),), |x| x).finish(cfg);
///     let num = bind((num.take_mut(),), |x| x).finish(cfg);
///     let num = bind((num.take_mut(),), |x| x).finish(cfg);
///
///     StaticTask::define()
///         .start_with_bind(
///             (num.borrow_mut(),),
///             |num: &mut &'static mut &'static mut &'static mut i32| {
///                 assert_eq!(****num, 42);
///             }
///         )
///         .priority(2)
///         .active(true)
///         .finish(cfg);
/// }
/// ```
)]
//!
//! The configuration system enforces the borrowing rules:
//!
#![doc = crate::tests::doc_test!(
/// ```rust,compile_fail,E0080
/// # use r3::{
/// #     kernel::{StaticTask, StaticTimer},
/// #     bind::bind,
/// #     time::Duration,
/// #     prelude::*,
/// # };
/// # type Objects = ();
/// const fn configure_app<C>(cfg: &mut Cfg<C>)
/// where
///     C: ~const traits::CfgBase<System = System> +
///        ~const traits::CfgTask +
///        ~const traits::CfgTimer,
/// {
///     let count = bind((), || 0).finish(cfg);
///     StaticTimer::define()
///         .start_with_bind((count.borrow_mut(),), |count: &mut i32| {})
///         .period(Duration::from_millis(50))
///         .delay(Duration::ZERO)
///         .active(true)
///         .finish(cfg);
///
///     StaticTask::define()
///         // ERROR: `count` is already mutably borrowed by the timer
///         .start_with_bind((count.borrow_mut(),), |count: &mut i32| {})
///         .priority(2)
///         .active(true)
///         .finish(cfg);
/// }
/// ```
)]
use r3_core::kernel::{cfg, raw, raw_cfg};

pub use r3_core::bind::{
    Bind, BindBorrow, BindBorrowMut, BindDefiner, BindRef, BindTable, BindTake, BindTakeMut,
    BindTakeRef, Binder, ExecutableDefiner, ExecutableDefinerExt, FnBind, FnBindNever, UnzipBind,
    INIT_HOOK_PRIORITY,
};

/// A shorthand for [`Bind`][]`::`[`define`][1]`().`[`init_with_bind`][2]`(...)`.
///
/// See [the module-level documentation][3] for an example.
///
/// [1]: Bind::define
/// [2]: BindDefiner::init_with_bind
/// [3]: self#examples
#[inline]
pub const fn bind<System, Binder, Func>(
    binder: Binder,
    func: Func,
) -> BindDefiner<System, Binder, Func> {
    Bind::define().init_with_bind(binder, func)
}

/// A shorthand for
/// [`Bind`][]`::`[`define`][1]`().`[`init`][2]`(`[`MaybeUninit::uninit`][4]`).`[`finish`][5]`(cfg)`.
///
/// This can be used to create a storage with the `'static` lifetime duration
/// that is initialized lazily.
///
/// See [the module-level documentation][3] for an example.
///
/// # Example
///
/// The following example uses `bind_uninit` as an alternative to
/// [`cortex_m::singleton!`][6] with no runtime checking.
///
#[doc = crate::tests::doc_test!(
/// ```rust
/// use r3::{
///     kernel::{StaticTask, StaticTimer},
///     bind::{bind, bind_uninit},
///     time::Duration,
///     prelude::*,
/// };
/// use core::mem::MaybeUninit as Mu;
///
/// # type Objects = ();
/// const fn configure_app<C>(cfg: &mut Cfg<C>)
/// where
///     C: ~const traits::CfgBase<System = System>,
/// {
///     bind(
///         (bind_uninit(cfg).take_mut(), bind_uninit(cfg).take_mut()),
///         |cell0: &'static mut Mu<_>, cell1: &'static mut Mu<_>| {
///             // Put a value in `cell0` and get a `'static` reference to it
///             let ref0: &'static mut i32 = cell0.write(42);
///
///             // And so on
///             let ref1: &'static mut &'static mut i32 = cell1.write(ref0);
///
///             assert_eq!(ref1, &mut &mut 42);
///             # exit(0);
///         }
///     ).unpure().finish(cfg);
/// }
/// ```
)]
///
/// [1]: Bind::define
/// [2]: BindDefiner::init
/// [3]: self#examples
/// [4]: core::mem::MaybeUninit::uninit
/// [5]: BindDefiner::finish
/// [6]: https://docs.rs/cortex-m/0.7.4/cortex_m/macro.singleton.html
#[inline]
pub const fn bind_uninit<'pool, T, C>(
    cfg: &mut cfg::Cfg<'pool, C>,
) -> Bind<'pool, C::System, core::mem::MaybeUninit<T>>
where
    T: 'static,
    C: ~const raw_cfg::CfgBase,
    C::System: raw::KernelBase + cfg::KernelStatic,
{
    // Safety: `MaybeUninit` is safe to leave uninitialized
    unsafe { Bind::define().uninit_unchecked().finish(cfg) }
}

/// A shorthand for
/// [`Bind`][]`::`[`define`][1]`().`[`init`][2]`(`[`default`][3]`)`.
///
/// [1]: Bind::define
/// [2]: BindDefiner::init_with_bind
/// [3]: core::default::default
///
/// # Example
///
#[doc = crate::tests::doc_test!(
/// ```rust
/// use r3::{bind::{bind, bind_default}, prelude::*,};
///
/// # type Objects = ();
/// const fn configure_app<C>(cfg: &mut Cfg<C>)
/// where
///     C: ~const traits::CfgBase<System = System>,
/// {
///     let b = bind_default(cfg);
///     bind((b.borrow(),), |b: &Option<i32>| {
///         assert!(b.is_none());
/// #       exit(0);
///     }).unpure().finish(cfg);
/// }
/// ```
)]
#[inline]
pub const fn bind_default<'pool, T, C>(cfg: &mut cfg::Cfg<'pool, C>) -> Bind<'pool, C::System, T>
where
    T: Default + 'static,
    C: ~const raw_cfg::CfgBase,
    C::System: raw::KernelBase + cfg::KernelStatic,
{
    Bind::define().init(Default::default).finish(cfg)
}
