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
///     bind::Bind,
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
///     let count = Bind::define().init(|| 0).finish(cfg);
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
///     Bind::define().unpure().init_with_bind(
///         (count.borrow_mut(),),
///         |count: &mut i32| {
///             *count = 5;
///         },
///     ).finish(cfg);
///
///     // Create a binding
///     let num = Bind::define().init(|| 42).finish(cfg);
///
///     // Then create a reference to it, a reference to the reference,
///     // and so on.
///     let num = Bind::define()
///         .init_with_bind((num.take_mut(),), |x| x).finish(cfg);
///     let num = Bind::define()
///         .init_with_bind((num.take_mut(),), |x| x).finish(cfg);
///     let num = Bind::define()
///         .init_with_bind((num.take_mut(),), |x| x).finish(cfg);
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
/// #     bind::Bind,
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
///     let count = Bind::define().init(|| 0).finish(cfg);
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
pub use r3_core::bind::{
    Bind, BindBorrow, BindBorrowMut, BindDefiner, BindRef, BindTable, BindTake, BindTakeMut,
    BindTakeRef, Binder, ExecutableDefiner, ExecutableDefinerExt, FnBind, UnzipBind,
    INIT_HOOK_PRIORITY,
};
