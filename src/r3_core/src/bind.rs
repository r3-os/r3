#![doc = __internal_module_doc!("crate", "", "")]
#![doc = include_str!("./common.md")]

/// The table of contents for [`__internal_module_doc`][].
#[rustfmt::skip]
#[doc(hidden)]
pub macro __internal_module_doc_toc() {r#"
- [Binders](#binders)
- [Initialization Order](#initialization-order)
- [Planned Features](#planned-features) "#} // No EOL at the end

/// The part of the module-level documentation shared between `r3::bind` and
/// `r3_core::bind`. This is necessary because `r3::bind` itself isn't a
/// re-export of `r3_core::bind`, but it's desirable for it to have the same
/// documentation.
#[rustfmt::skip]
#[doc(hidden)]
#[macropol::macropol] // Replace `$metavariables` in literals and doc comments
pub macro __internal_module_doc($r3_core:expr, $admonitions:expr, $toc:expr) {
    r#"
Bindings ([`Bind`][]), a static storage with [runtime initialization][1] and
[configuration-time][2] borrow checking.

$admonitions

Bindings are essentially fancy global variables defined in a kernel
configuration. They are defined by [`Bind::define`][] and initialized by
provided closures at runtime. They can be consumed or borrowed by the entry
points of [executable kernel objects][4] or the initializers of another
bindings, with a dependency graph explicitly defined by the passing of
[*binders*][15].

The configuration system tracks the usage of bindings and employs static checks
to ensure that the borrowing rules are followed by the users of the bindings. It
aborts the compilation if the rules may be violated.

Bindings use hunks ([`Hunk`][3]) as a storage for their contents. Bindings are
initialized in [startup hooks][14], where [CPU Lock][17] is active, <!--
[ref:startup_hook_cpu_lock_active] --> and therefore most kernel services
are unavailable.

<div class="admonition-follows"></div>

> **Relation to Other Specifications:** [*Resources*][8] in [RTIC 1][7] serve
> the similar need with a quite different design.
> In R3, bindings are defined in modular, encapsulated configuration functions
> and associated to various kernel objects. The configuration system takes all
> definitions and figures out the correct initialization order.
> In RTIC, all resources are defined in one place and initialized by an
> application-provided `#[init]` function.

$toc

# Binders

A *binder* ([`Binder`][]) represents a specific borrow mode of a binding. A
configuration function creates a binding by calling one of [`Bind`][]'s methods
and uses it in the definition of another object where the binding is intended to
be consumed, i.e., borrowed or moved out by its entry point or initializer. The
type a binder produces is called its *materialized* form.

```rust,ignore
use r3::{bind::bind, kernel::StaticTimer, prelude::*};

let count = bind((), || 0).finish(cfg);
//               ^^  ^^
//            .--'    '--------,
//       no binders  no materialized values

StaticTimer::define()
    // n.b. `(x,)` is a one-element tuple
    .start_with_bind((count.borrow_mut(),), |count: &mut i32| {
        //            ^^^^^^^^^^^^^^^^^^            ^^^^^^^^
        //                    |                         |
        //         BindBorrowMut<'_, _, i32>      &'call mut i32
        //                  gives...          for some lifetime 'call
    })
    .finish(b);
```

The only way for safe code to make a binding available to runtime code in a
meaningful way is to include a binder in a dependency list (e.g., the `binder`
parameter of [`ExecutableDefinerExt::start_with_bind`][]) as shown above or to
call [`Bind::as_ref`][] to create a [`BindRef`][], which is directly consumable
in runtime code. Most binders can't escape from a configuration function.

The following table lists all provided binders:

|     `Bind::`     |         Type        |     Confers      | On binding | On executable |
| ---------------- | ------------------- | ---------------- | :--------: | :-----------: |
| [`borrow`][]     | [`BindBorrow`][]    | `&'call T`       |     ✓      |       ✓       |
| [`borrow_mut`][] | [`BindBorrowMut`][] | `&'call mut T`   |     ✓      |       ✓       |
| [`take_ref`][]   | [`BindTakeRef`][]   | `&'static T`     |     ✓      |       ✓       |
| [`take_mut`][]   | [`BindTakeMut`][]   | `&'static mut T` |     ✓      |               |
| [`take`][]       | [`BindTake`][]      | `T`              |     ✓      |               |
| [`as_ref`][]     | [`BindRef`][]       | `&'static T`     |            |       ✓       |

[`borrow`]: Bind::borrow
[`borrow_mut`]: Bind::borrow_mut
[`take_ref`]: Bind::take_ref
[`take_mut`]: Bind::take_mut
[`take`]: Bind::take
[`as_ref`]: Bind::as_ref

- The **`Bind::`** column shows the methods to create the binders.

- The **Type** column shows the types representing the binders.

- The **Confers** column shows the respective materialized forms of the binders.
  The lifetime `'call` represents the call duration of the consuming function.
   - For `&'call T` and `&'call mut T`, the caller has the freedom of choosing
     an arbitrary lifetime, so the consuming function must be generic over any
     lifetimes. In function and closure parameters, reference types without
     explicit lifetimes are automatically made generic in this way owing to [the
     lifetime elision rules][16].

- The **On binding** column shows which types of binders can be consumed by
  another binding's initializer via [`BindDefiner::init_with_bind`][].

- The **On executable** column shows which types of binders can be consumed
  by [executable objects][10], viz., [tasks][11], [interrupt handlers][12], and
  [timers][13], via [`ExecutableDefinerExt::start_with_bind`][].
    - An executable object may execute its entry point for multiple times
      throughout its lifetime. For this reason, an executable object is not
      allowed to consume `BindTake` (which moves out the value) or `BindTakeMut`
      (which mutably borrows the value indefinitely).

# Initialization Order

The configuration system determines the initialization order of the defined
bindings by [topological sorting][5] with a preference toward the definition
order. The specific algorithm is not a part of the stability guarantee.

# Planned Features

The following features are planned and may be implemented in the future:

- Reusing the storage of a binding whose lifetime has ended by having its
  contents moved out by [`BindTake`][] or completing its last borrow.
  <!-- [tag:bind_storage_reuse] -->

- Pruning unused bindings, unless they are marked as [`unpure`][6].
  <!-- [ref:unpure_binding] -->

- Phantom edges to enforce ordering between bindings.

- Pinning.

[1]: BindDefiner::init
[2]: $r3_core#static-configuration
[3]: crate::kernel::Hunk
[4]: ExecutableDefiner
[5]: https://en.wikipedia.org/wiki/Topological_sorting
[6]: BindDefiner::unpure
[7]: https://rtic.rs/1/book/en/
[8]: https://rtic.rs/1/book/en/by-example/resources.html
[9]: https://www.toppers.jp/index.html
[10]: ExecutableDefiner
[11]: crate::kernel::StaticTask
[12]: crate::kernel::StaticInterruptHandler
[13]: crate::kernel::StaticTimer
[14]: crate::kernel::StartupHook
[15]: #binders
[16]: https://doc.rust-lang.org/1.58.1/reference/lifetime-elision.html#lifetime-elision-in-functions
[17]: $r3_core#system-states
"#
}

use core::{cell::UnsafeCell, marker::PhantomData, mem::MaybeUninit};

use crate::{
    closure::Closure,
    hunk::Hunk,
    kernel::{self, cfg, prelude::*, raw, raw_cfg, StartupHook},
    utils::{refcell::RefCell, ComptimeVec, ConstAllocator, Init, PhantomInvariant, ZeroInit},
};

mod sorter;
mod tests_impls;

/// The priority of the [startup hooks][1] used to initialize [bindings][2].
///
/// [1]: crate::kernel::StartupHook
/// [2]: Bind
pub const INIT_HOOK_PRIORITY: i32 = 0x4000_0000;

// Storage for the bindings
// ----------------------------------------------------------------------------

#[doc(hidden)]
#[repr(transparent)]
pub struct BindData<T>(UnsafeCell<MaybeUninit<T>>);

// Safety: Thread safety is dealt with by the binder creation methods
unsafe impl<T> Sync for BindData<T> {}
unsafe impl<T> Send for BindData<T> {}

/// A type alias of `Hunk` specialized for [`Bind`].
type BindHunk<System, T> = Hunk<System, BindData<T>>;

impl<T> BindData<T> {
    #[inline]
    unsafe fn assume_init_ref(&self) -> &T {
        unsafe { (*self.0.get()).assume_init_ref() }
    }

    #[inline]
    // Forming a mutable borrow from an immutable input is okay because we go
    // through `UnsafeCell`
    #[allow(clippy::mut_from_ref)]
    unsafe fn assume_init_mut(&self) -> &mut T {
        unsafe { (*self.0.get()).assume_init_mut() }
    }
}

// Safety: Zero-initialization is valid for `MaybeUninit`
unsafe impl<T> ZeroInit for BindData<T> {}

// Main configuration interface
// ----------------------------------------------------------------------------

// [ref:unsized_maybe_uninit] prevents `T: ?Sized`
/// A defined binding.
///
/// The configuration-time metadata is stored in a pool with lifetime `'pool`.
pub struct Bind<'pool, System, T> {
    hunk: BindHunk<System, T>,
    bind_registry: &'pool RefCell<CfgBindRegistry>,
    bind_i: usize,
}

impl<System, T> Copy for Bind<'_, System, T> {}
impl<System, T> const Clone for Bind<'_, System, T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

/// A [binder][1] that gives `&T` to a bound function.
///
/// Created by [`Bind::borrow`][].
///
/// [1]: index.html#binders
pub struct BindBorrow<'pool, System, T>(Bind<'pool, System, T>);

/// A [binder][1] that gives `&mut T` to a bound function.
///
/// Created by [`Bind::borrow_mut`][].
///
/// [1]: index.html#binders
pub struct BindBorrowMut<'pool, System, T>(Bind<'pool, System, T>);

/// A [binder][1] that gives `T` to a bound function.
///
/// Created by [`Bind::take`][].
///
/// [1]: index.html#binders
pub struct BindTake<'pool, System, T>(Bind<'pool, System, T>);

/// A [binder][1] that gives `&'static T` to a bound function.
///
/// Created by [`Bind::take_ref`][].
///
/// [1]: index.html#binders
pub struct BindTakeRef<'pool, System, T>(Bind<'pool, System, T>);

/// A [binder][1] that gives `&'static mut T` to a bound function.
///
/// Created by [`Bind::take_mut`][].
///
/// [1]: index.html#binders
pub struct BindTakeMut<'pool, System, T>(Bind<'pool, System, T>);

/// A reference to a [binding][1]. Doubles as a [binder][1].
///
/// Created by [`Bind::as_ref`][].
///
/// It doesn't provide access to the contents by itself because it could be
/// used before the binding is initialized. Index [`BindTable`][] by this type to
/// borrow the contents as `&'static T`.
///
/// [1]: Bind
/// [2]: index.html#binders
pub struct BindRef<System, T>(BindHunk<System, T>);

impl<System, T> Copy for BindRef<System, T> {}
impl<System, T> const Clone for BindRef<System, T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

// `BindDefiner` doesn't contain `T`, so this `impl` must use a concrete `T`
// for `define` to be usable
impl<'pool, System> Bind<'pool, System, ()> {
    /// Construct a `BindDefiner` to define a binding in [a configuration
    /// function](crate#static-configuration).
    pub const fn define() -> BindDefiner<
        System,
        private_bind_definer::BinderUnspecified,
        private_bind_definer::FuncUnspecified,
    > {
        BindDefiner::new()
    }
}

/// # Binders
///
/// The following methods are used to construct a [*binder*][1], which is a
/// reference to a binding with a specific borrowing mode.
///
/// [1]: Binder
impl<'pool, System, T> Bind<'pool, System, T> {
    /// Create a [`BindBorrow`][] binder, which gives `&T` to a bound function.
    pub const fn borrow(&self) -> BindBorrow<'pool, System, T>
    where
        T: Sync,
    {
        BindBorrow(*self)
    }

    /// Create a [`BindBorrowMut`][] binder, which gives `&mut T` to a bound
    /// function.
    pub const fn borrow_mut(&self) -> BindBorrowMut<'pool, System, T>
    where
        T: Send,
    {
        BindBorrowMut(*self)
    }

    /// Create a [`BindTake`][] binder, which gives `T` to a bound function.
    pub const fn take(&self) -> BindTake<'pool, System, T>
    where
        T: Send,
    {
        BindTake(*self)
    }

    /// Create a [`BindTakeRef`][] binder, which gives `&'static T` to a bound
    /// function.
    pub const fn take_ref(&self) -> BindTakeRef<'pool, System, T>
    where
        T: Sync,
    {
        BindTakeRef(*self)
    }

    /// Create a [`BindTakeMut`][] binder, which gives `&'static mut T` to a
    /// bound function.
    pub const fn take_mut(&self) -> BindTakeMut<'pool, System, T>
    where
        T: Send,
    {
        BindTakeMut(*self)
    }

    /// Construct a [`BindRef`][], which can be used to get `&'static T` from a
    /// [`BindTable`][]`<System>`.
    ///
    /// `BindRef` doubles as a [binder][2] that gives `&'static T` in a bound
    /// [executable object][1].
    ///
    /// The configuration system can't track the usages of `BindRef` (note the
    /// lack of a lifetime parameter in its definition). As such, merely calling
    /// this method counts as a use of the binding whether the returned
    /// `BindRef` is actually used or not.
    ///
    /// [1]: ExecutableDefiner
    /// [2]: index.html#binders
    pub const fn as_ref(&self) -> BindRef<System, T>
    where
        T: Sync,
    {
        self.bind_registry.borrow_mut().binds[self.bind_i]
            .users
            .push((BindUsage::Executable, BindBorrowType::TakeRef));
        BindRef(self.hunk)
    }
}

/// # Miscellaneous Methods
impl<'pool, System, T> Bind<'pool, System, T> {
    /// Get the raw [`Hunk`][] providing the backing store for this binding.
    ///
    /// The referent type of the returned `Hunk` is an opaque type that has the
    /// same representation as `T` (because of `#[repr(transparent)]`). You
    /// can use [`Hunk::transmute`][] to reinterpret it as you see fit.
    ///
    /// Calling this method alone does not extend the binding's lifetime nor
    /// prevent conflicting borrows from occurring. A dummy binding can be used
    /// to express the manner in which the raw hunk is used outside the
    /// configuration system's knowledge. For example, if the raw hunk is going
    /// to be mutably borrowed, the caller of this method should create a dummy
    /// [unpure][1] binding that consumes the binding by [`take_mut`][2].
    ///
    /// [1]: BindDefiner::unpure
    /// [2]: Bind::take_mut
    pub const fn hunk(&self) -> BindHunk<System, T> {
        self.hunk
    }
}

/// The definer (static builder) for [`Bind`].
#[doc = include_str!("./common.md")]
#[must_use = "must call `finish()` to complete registration"]
pub struct BindDefiner<System, Binder, Func> {
    _phantom: PhantomInvariant<System>,
    binder: Binder,
    func: Option<Func>,
}

mod private_bind_definer {
    pub struct BinderUnspecified;
    pub struct FuncUnspecified;
}

impl<System>
    BindDefiner<
        System,
        private_bind_definer::BinderUnspecified,
        private_bind_definer::FuncUnspecified,
    >
{
    const fn new() -> Self {
        Self {
            _phantom: Init::INIT,
            binder: private_bind_definer::BinderUnspecified,
            func: Some(private_bind_definer::FuncUnspecified),
        }
    }
}

/// # Specifying the initializer function
///
/// One of the following methods should be used to specify the initializer for
/// the binding being defined.
impl<System>
    BindDefiner<
        System,
        private_bind_definer::BinderUnspecified,
        private_bind_definer::FuncUnspecified,
    >
{
    /// Use the function to initialize the binding contents.
    pub const fn init<Func>(self, func: Func) -> BindDefiner<System, (), Func> {
        BindDefiner {
            func: Some(func),
            binder: (),
            ..self
        }
    }

    /// Use the function with dependency to initialize the binding contents.
    pub const fn init_with_bind<Binder, Func>(
        self,
        binder: Binder,
        func: Func,
    ) -> BindDefiner<System, Binder, Func> {
        BindDefiner {
            func: Some(func),
            binder,
            ..self
        }
    }

    /// Zero-initialize the binding contents.
    pub const fn zeroed<T: ZeroInit>(self) -> BindDefiner<System, (), FnBindNever<T>> {
        // Safety: `T: ZeroInit` means it's safe to zero-initialize
        unsafe { self.zeroed_unchecked() }
    }

    /// Zero-initialize the binding contents even if it might be unsafe.
    ///
    /// # Safety
    ///
    /// If zero initialization is not a valid bit pattern for `T`, accessing the
    /// contents may result in an undefined behavior.
    pub const unsafe fn zeroed_unchecked<T>(self) -> BindDefiner<System, (), FnBindNever<T>> {
        BindDefiner {
            // Hunk pool is zero-initialized by default
            // [ref:hunk_pool_is_zeroed]
            func: None,
            binder: (),
            ..self
        }
    }

    /// Skip the initialization of the binding contents even if it might be
    /// unsafe.
    ///
    /// # Safety
    ///
    /// If the uninitialized state is not valid for `T`, accessing the
    /// contents may result in an undefined behavior.
    pub const unsafe fn uninit_unchecked<T>(self) -> BindDefiner<System, (), FnBindNever<T>> {
        // FIXME: True uninitialized hunks would be more desirable
        // Safety: Upheld by the caller
        unsafe { self.zeroed_unchecked() }
    }
}

/// # Optional Parameters
impl<System, Binder, Func> BindDefiner<System, Binder, Func> {
    /// Indicate that the evaluation of the initializer may cause a side-effect
    /// that the dependency solver must not remove implicitly.
    ///
    /// <div class="admonition-follows"></div>
    ///
    /// > **Unimplemented:** Pruning unused bindings is not implemented yet.
    /// > Therefore, this method is no-op. <!-- [ref:unpure_binding] -->
    ///
    pub const fn unpure(self) -> Self {
        // TODO: [tag:unpure_binding] Mark impurity
        self
    }
}

/// # Finalization
///
/// The following method defines a binding using the provided parameter.
impl<System, Binder, Func> BindDefiner<System, Binder, Func> {
    /// Complete the definition of a binding, returning a reference to it.
    pub const fn finish<'pool, C>(
        self,
        cfg: &mut cfg::Cfg<'pool, C>,
    ) -> Bind<'pool, System, <Func as FnBind<Binder>>::Output>
    where
        C: ~const raw_cfg::CfgBase<System = System>,
        System: raw::KernelBase + cfg::KernelStatic,
        Func: ~const FnBind<Binder>,
    {
        let hunk = BindHunk::define().zeroed().finish(cfg);

        let bind_registry = &cfg.shared.bind_registry;
        let bind_i = bind_registry.borrow().binds.len();

        // Construct the initializer for the binding being created
        let initializer = if let Some(func) = self.func {
            let mut ctx = CfgBindCtx {
                _phantom: &(),
                usage: BindUsage::Bind(bind_i),
            };
            let initializer = func.bind(self.binder, &mut ctx);
            Some(Closure::from_fn_const(
                #[inline]
                move || {
                    let output = initializer();
                    // Safety: There's no conflicting borrows
                    unsafe { hunk.0.get().write(MaybeUninit::new(output)) };
                },
            ))
        } else {
            // `(None, ())` should be harmless to `forget`
            core::mem::forget((self.func, self.binder));
            None
        };

        {
            let mut bind_registry = bind_registry.borrow_mut();
            assert!(bind_i == bind_registry.binds.len());
            let allocator = bind_registry.binds.allocator().clone();
            bind_registry
                .binds
                .push(CfgBindInfo::new(initializer, allocator));
        }

        Bind {
            hunk,
            bind_registry,
            bind_i,
        }
    }
}

/// An internal utility to divide a [`Bind`][] into smaller parts.
///
/// <!-- [tag:bind_divide_by_take_mut] --> This is currently implemented by
/// introducing artificial dependency edges to the produced parts, which is
/// prone to false dependencies due to conservatism. This may be relaxed in the
/// future.
/// `BindTakeMut` forces the value to stay indefinitely, but fortunately in the
/// current implementation this doesn't have an impact on the memory usage
/// because storage reuse isn't implemented yet. <!-- [ref:bind_storage_reuse]
/// -->
///
/// ```text
///                               TakeRef
///                                  ,---> emitter0 (T0)
///             TakeMut              |
///    original ------> collector ---+---> emitter1 (T1)
///  (T0, T1, ...)                   |
///                                  '---> ...
///
/// ```
struct DivideBind<'pool, System, T> {
    original_bind: Bind<'pool, System, T>,
    collector_bind_i: usize,
}

impl<'pool, System, T> DivideBind<'pool, System, T> {
    const fn new(original_bind: Bind<'pool, System, T>) -> Self {
        let collector_bind_i;

        {
            let mut bind_registry = original_bind.bind_registry.borrow_mut();

            // Create the "collector" binding
            collector_bind_i = bind_registry.binds.len();
            let allocator = bind_registry.binds.allocator().clone();
            bind_registry.binds.push(CfgBindInfo::new(None, allocator));

            // original → collector
            // Using `TakeMut` here is a conservative choice.
            // [ref:bind_divide_by_take_mut]
            bind_registry.binds[original_bind.bind_i]
                .users
                .push((BindUsage::Bind(collector_bind_i), BindBorrowType::TakeMut));
        }

        Self {
            original_bind,
            collector_bind_i,
        }
    }

    /// Get the original binding's `BindHunk`.
    const fn original_hunk(&self) -> BindHunk<System, T> {
        self.original_bind.hunk
    }

    /// Create a new binding from the provided `BindHunk`, treating it as a
    /// portion of the original binding.
    ///
    /// # Safety
    ///
    /// - `hunk` must represent a subset of [`Self::original_hunk`].
    /// - For a given original binding, there must be no overlaps between the
    ///   `hunk`s passed to this method.
    ///
    const unsafe fn slice<U>(&self, hunk: BindHunk<System, U>) -> Bind<'pool, System, U> {
        let emitter_bind_i;

        {
            let mut bind_registry = self.original_bind.bind_registry.borrow_mut();

            // Create the "collector" binding
            emitter_bind_i = bind_registry.binds.len();
            let allocator = bind_registry.binds.allocator().clone();
            bind_registry.binds.push(CfgBindInfo::new(None, allocator));

            // collector → emitter
            bind_registry.binds[self.collector_bind_i]
                .users
                .push((BindUsage::Bind(emitter_bind_i), BindBorrowType::TakeRef));
        }

        Bind {
            hunk,
            bind_registry: self.original_bind.bind_registry,
            bind_i: emitter_bind_i,
        }
    }
}

/// An extension trait for destructing [`Bind`][]`<_, (T0, T1, ...)>` into
/// individual bindings `(Bind<_, T0>, Bind<_, T1>, ...)`.
///
/// <div class="admonition-follows"></div>
///
/// > **Temporary Restrictions:** Currently the destruction makes the original
/// > value completely inaccessible owing to it being implemented by
/// > [`BindTakeMut`][]. Essentially, this divides the timeline into two parts:
/// > the first part where only the pre-destruction binding (`Bind<_, (T0, T1,
/// > ...)>`) can be borrowed and the second part where only the
/// > post-destruction bindings (`Bind<_, T0>, Bind<_, T1>, ...`) can be
/// > borrowed. <!-- [ref:bind_divide_by_take_mut] -->
///
/// # Examples
///
/// ```rust
/// #![feature(const_trait_impl)]
/// #![feature(const_mut_refs)]
/// use r3_core::{bind::Bind, kernel::{Cfg, traits}, prelude::*};
///
/// const fn configure_app<C>(cfg: &mut Cfg<C>)
/// where
// `~const CfgBase` not implied due to [ref:const_supertraits]
///     C: ~const traits::CfgBase + ~const traits::CfgStatic,
/// {
///     let values = Bind::define().init(|| (12, 34)).finish(cfg);
///     let (value0, value1) = values.unzip();
///     Bind::define()
///         .init_with_bind((value0.take_mut(),), |x: &mut i32| assert_eq!(*x, 12))
///         .unpure()
///         .finish(cfg);
///     Bind::define()
///         .init_with_bind((value1.take_mut(),), |x: &mut i32| assert_eq!(*x, 34))
///         .unpure()
///         .finish(cfg);
/// }
/// ```
///
/// # Stability
///
/// This trait is covered by the application-side API stability guarantee.
/// External implementation of this trait is not allowed.
#[doc = include_str!("./common.md")]
pub trait UnzipBind {
    type Target;
    /// Destruct [`Bind`][] into individual bindings.
    fn unzip(self) -> Self::Target;
}

/// A helper trait to provide the field offsets of tuple types.
trait TupleFieldOffsets {
    const FIELD_OFFSETS: &'static [usize];
}

macro_rules! impl_unzip_bind_for_tuples {
    ( @start $($x:tt)* ) => {
        impl_unzip_bind_for_tuples! { @iter [] [$($x)*] }
    };

    // inductive case
    ( @iter
        [$(($FieldI:ident, $I:tt))*]
        [$next_head:tt $($next_tail:tt)*]
    ) => {
        impl_unzip_bind_for_tuples! { @iter [$(($FieldI, $I))* $next_head] [$($next_tail)*] }

        impl<$( $FieldI, )*> TupleFieldOffsets for ($( $FieldI, )*) {
            const FIELD_OFFSETS: &'static [usize] = &[
                $( memoffset::offset_of_tuple!(Self, $I) ),*
            ];
        }
        impl<
            System,
            $( $FieldI, )*
        > TupleFieldOffsets for Bind<'_, System, ($( $FieldI, )*)> {
            const FIELD_OFFSETS: &'static [usize] = <($( $FieldI, )*)>::FIELD_OFFSETS;
        }

        impl<
            'pool,
            System,
            $( $FieldI, )*
        > const UnzipBind for Bind<'pool, System, ( $( $FieldI, )* )>
        {
            type Target = ( $( Bind<'pool, System, $FieldI>, )* );

            fn unzip(self) -> Self::Target {
                #[allow(unused_unsafe)] // for `Bind<'_, _, ()>`
                #[allow(clippy::unused_unit)] // for `Bind<'_, _, ()>`
                unsafe {
                    let divide = DivideBind::new(self);
                    let hunk = divide.original_hunk();
                    let _ = hunk;

                    ($(
                        divide.slice::<$FieldI>(
                            hunk.transmute::<u8>()
                                .wrapping_offset(Self::FIELD_OFFSETS[$I] as isize)
                                .transmute()
                        ),
                    )*)
                }
            }
        }
    }; // end of macro arm

    // base case
    ( @iter [$($_discard:tt)*] [] ) => {}
}

seq_macro::seq!(I in 0..16 { impl_unzip_bind_for_tuples! { @start #( (Field~I, I) )* } });

impl<'pool, const LEN: usize, System, T> const UnzipBind for Bind<'pool, System, [T; LEN]> {
    type Target = [Bind<'pool, System, T>; LEN];

    fn unzip(self) -> Self::Target {
        unsafe {
            let divide = DivideBind::new(self);
            let hunk = divide.original_hunk();

            // `core::array::from_fn` isn't `const fn` [ref:const_array_from_fn]
            let mut out =
                ComptimeVec::new_in(self.bind_registry.borrow().binds.allocator().clone());

            // `for` loops are unusable in `const fn` [ref:const_for]
            let mut i = 0;
            while i < LEN {
                out.push(divide.slice(hunk.transmute::<BindData<T>>().wrapping_offset(i as isize)));
                i += 1;
            }

            out.to_array()
        }
    }
}

// Runtime binding registry
// ----------------------------------------------------------------------------

/// Represents a permission to dereference [`BindRef`][].
///
/// # Example
///
/// ```rust
/// #![feature(const_trait_impl)]
/// #![feature(const_mut_refs)]
/// use r3_core::{
///     bind::{Bind, BindRef, BindTable},
///     kernel::{cfg::Cfg, traits, StaticTask},
///     prelude::*,
/// };
///
/// const fn configure_app<C>(cfg: &mut Cfg<C>)
/// where
///     C: ~const traits::CfgBase +
///        ~const traits::CfgTask,
///     C::System: traits::KernelStatic,
/// {
///     let foo = Bind::define().init(|| {
///         // `BindTable::get()` will fail because some bindings might not
///         // be initialized at this point
///         assert!(BindTable::<C::System>::get().is_err());
///
///         42
///     }).finish(cfg).as_ref();
///
///     StaticTask::define()
///         .start(move || {
///             // Task code can get `BindTable` because tasks can only
///             // run after the boot phase is complete
///             let bt = BindTable::get().unwrap();
///             assert_eq!(bt[foo], 42);
///         })
///         .priority(2)
///         .active(true)
///         .finish(cfg);
/// }
/// ```
pub struct BindTable<System> {
    _phantom: PhantomInvariant<System>,
}

/// Error type for [`BindTable::get`][].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GetBindTableError {
    /// [The boot phase][] is not complete.
    ///
    /// [the boot phase]: crate#threads
    BadContext,
}

impl<System> BindTable<System>
where
    System: raw::KernelBase + cfg::KernelStatic,
{
    /// Get a reference to `BindTable` if [the boot phase][] is complete.
    ///
    /// Returns `Err(BadContext)` if the boot phase hasn't completed yet, and
    /// therefore it's unsafe to dereference [`BindRef`]s.
    ///
    /// [the boot phase]: crate#threads
    #[inline]
    pub fn get() -> Result<&'static Self, GetBindTableError> {
        if System::is_boot_complete() {
            Ok(&Self {
                _phantom: Init::INIT,
            })
        } else {
            Err(GetBindTableError::BadContext)
        }
    }

    /// Get a reference to `BindTable` without checking if it's safe to do so
    /// in the current context.
    ///
    /// # Safety
    ///
    /// The returned reference may be used to borrow binding contents that are
    /// uninitialized or being mutably borrowed by a binding initializer.
    #[inline]
    pub const unsafe fn get_unchecked() -> &'static Self {
        &Self {
            _phantom: Init::INIT,
        }
    }
}

impl<System, T> core::ops::Index<BindRef<System, T>> for BindTable<System>
where
    System: raw::KernelBase + cfg::KernelStatic,
    T: 'static,
{
    type Output = T;

    #[inline]
    fn index(&self, index: BindRef<System, T>) -> &Self::Output {
        // Safety: The possession of `BindRef` and `BindTable` indicates it's
        // safe to do so
        unsafe { BindHunk::as_ref(index.0).assume_init_ref() }
    }
}

// Configuration-time binding registry
// ----------------------------------------------------------------------------

pub(crate) struct CfgBindRegistry {
    binds: ComptimeVec<CfgBindInfo>,
}

struct CfgBindInfo {
    /// The initializer for the binder. It'll be registered as a startup hook
    /// on finalization.
    initializer: Option<Closure>,
    /// The uses of this binding.
    users: ComptimeVec<(BindUsage, BindBorrowType)>,
}

impl CfgBindRegistry {
    pub const fn new_in(allocator: ConstAllocator) -> Self {
        Self {
            binds: ComptimeVec::new_in(allocator),
        }
    }

    /// Determine the initialization order of the defined bindings and register
    /// startup hooks to initialize them at runtime.
    pub const fn finalize<C>(&mut self, cfg: &mut cfg::Cfg<C>)
    where
        C: ~const raw_cfg::CfgBase,
    {
        struct Callback<'a> {
            binds: &'a [CfgBindInfo],
            bind_init_order: ComptimeVec<usize>,
        }

        impl const sorter::SorterCallback for Callback<'_> {
            fn push_bind_order(&mut self, bind_i: usize) {
                self.bind_init_order.push(bind_i);
            }

            fn report_error(&mut self, e: sorter::SorterError<'_>) {
                // TODO: Collect all errors and report at once
                // [tag:bind_finalization_immediate_panic] The errors are
                // reported by panicking immediately for now
                match e {
                    sorter::SorterError::BindCycle { bind_is } => {
                        let _ = bind_is; // TODO: Display the bindings' names
                        panic!("the binding initialization order contains a cycle");
                    }
                    sorter::SorterError::ConflictingIndefiniteBorrow { bind_i } => {
                        let _ = bind_i; // TODO: Display the binding's name
                        panic!("conflicting indefinite borrows");
                    }
                }
            }

            fn num_binds(&self) -> usize {
                self.binds.len()
            }

            fn bind_users(&self, bind_i: usize) -> &[(BindUsage, BindBorrowType)] {
                &self.binds[bind_i].users
            }
        }

        let allocator = self.binds.allocator();

        let mut callback = Callback {
            binds: &self.binds,
            bind_init_order: ComptimeVec::with_capacity_in(self.binds.len(), allocator.clone()),
        };

        sorter::sort_bindings(
            &mut callback,
            &mut ComptimeVec::repeat_in(allocator.clone(), Init::INIT, self.binds.len()),
            &mut ComptimeVec::repeat_in(allocator.clone(), Init::INIT, self.binds.len()),
            &mut ComptimeVec::new_in(allocator.clone()),
            &mut ComptimeVec::new_in(allocator.clone()),
        );

        // Because of [ref:bind_finalization_immediate_panic], reaching here
        // means the operation was successful

        // `for` loops are barely useful in `const fn` at the moment
        // [ref:const_for]
        let mut i = 0;
        while i < callback.bind_init_order.len() {
            let bind_i = callback.bind_init_order[i];

            if let Some(initializer) = self.binds[bind_i].initializer {
                StartupHook::define()
                    .start(initializer)
                    .priority(INIT_HOOK_PRIORITY)
                    .finish(cfg);
            }

            i += 1;
        }
    }
}

impl CfgBindInfo {
    const fn new(initializer: Option<Closure>, allocator: ConstAllocator) -> Self {
        CfgBindInfo {
            initializer,
            users: ComptimeVec::new_in(allocator),
        }
    }
}

#[doc(hidden)]
pub struct CfgBindCtx<'pool> {
    _phantom: &'pool (),
    usage: BindUsage,
}

/// A place where a binding is consumed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BindUsage {
    /// The initializer of another binding.
    Bind(usize),
    /// An executable object (i.e., objects defined by [`ExecutableDefiner`]).
    Executable,
}

/// The manner in which a binding is consumed.
///
/// Note that this represents the duration in which a binding is borrowed, not
/// the lifetime passed to an application-provided function. E.g.,
/// `BindBorrowMut` may give `&mut T` to a task entry point, but this is
/// represented by `BindBorrowType::TakeMut` because the task can be started
/// any time and repeatedly during the application's lifetime
/// [tag:borrow_is_indefinite_for_executable].
#[derive(Clone, Copy, PartialEq, Eq)]
enum BindBorrowType {
    /// Gives `&T` that is valid for the consumption duration.
    /// Invalid for executables [ref:borrow_is_indefinite_for_executable].
    Borrow,
    /// Gives `&mut T` that is valid for the consumption duration.
    /// Invalid for executables [ref:borrow_is_indefinite_for_executable].
    BorrowMut,
    /// Gives `T`. This is similar to [`Self::TakeMut`][] except that the
    /// storage may be freed up after the use. This is also similar to
    /// [`Self::BorrowMut`][] except that the binding is reverted to an
    /// uninitialized state, and the storage is available for reuse starting
    /// from the consuming function (whereas `BorrowMut` must wait until the
    /// completion of the consuming function).
    Take,
    /// Gives `&'static T`. This is an indefinite borrow.
    TakeRef,
    /// Gives `&'static mut T`. This is an indefinite borrow.
    TakeMut,
}

// Extensions for the definer objects
// ----------------------------------------------------------------------------

/// A trait for definer objects (static builders) for kernel objects that can
/// spawn a thread that executes after the execution of all startup hooks is
/// complete.
///
/// See [`ExecutableDefinerExt`][] for an extension trait providing a method
/// to specify a closure that consumes [binders][1].
///
/// [1]: index.html#binders
///
/// # Safety
///
/// At any point of time, the provided [`Closure`] must never be invoked by two
/// threads simultaneously. It can be called for multiple times, however.
pub unsafe trait ExecutableDefiner: Sized + private::Sealed {
    /// Use the specified function as the entry point of the executable object
    /// being defined.
    fn start(self, start: Closure) -> Self;
}

mod private {
    use super::*;

    pub trait Sealed {}

    impl<System: raw::KernelBase> const Sealed for kernel::task::TaskDefiner<System> {}
    impl<System: raw::KernelInterruptLine> const Sealed
        for kernel::interrupt::InterruptHandlerDefiner<System>
    {
    }
    impl<System: raw::KernelTimer> const Sealed for kernel::timer::TimerDefiner<System> {}
}

unsafe impl<System: raw::KernelBase> const ExecutableDefiner for kernel::task::TaskDefiner<System> {
    fn start(self, start: Closure) -> Self {
        self.start(start)
    }
}

unsafe impl<System: raw::KernelInterruptLine> const ExecutableDefiner
    for kernel::interrupt::InterruptHandlerDefiner<System>
{
    fn start(self, start: Closure) -> Self {
        self.start(start)
    }
}

unsafe impl<System: raw::KernelTimer> const ExecutableDefiner
    for kernel::timer::TimerDefiner<System>
{
    fn start(self, start: Closure) -> Self {
        self.start(start)
    }
}

// TODO: This probably can be moved to `r3`
/// An extension trait for [`ExecutableDefiner`]. Provides a method to
/// attach an entry point with materialized [bindings][1].
///
/// [1]: Bind
pub trait ExecutableDefinerExt {
    /// Use the specified function with dependency as the entry point of the
    /// executable object being defined.
    fn start_with_bind<Binder, Func: ~const FnBind<Binder, Output = ()>>(
        self,
        binder: Binder,
        func: Func,
    ) -> Self;
}

impl<T: ~const ExecutableDefiner> const ExecutableDefinerExt for T {
    fn start_with_bind<Binder, Func: ~const FnBind<Binder, Output = ()>>(
        self,
        binder: Binder,
        func: Func,
    ) -> Self {
        let mut ctx = CfgBindCtx {
            _phantom: &(),
            usage: BindUsage::Executable,
        };
        self.start(Closure::from_fn_const(func.bind(binder, &mut ctx)))
    }
}

// ----------------------------------------------------------------------------

/// A trait for closures that can receive [bindings][1] materialized through
/// specific [binders][4] (`Binder`).
///
/// `FnBind<(B0, B1, ...)>` is implemented for `impl for<'call>
/// FnOnce(M0<'call>, M1<'call>, ...) + Copy + Send + 'static`, where `Mn<'call>
/// == Bn::`[`Runtime`][2]`::`[`Target`][3]`<'call>` (`Bn`'s materialized form).
///
/// [1]: Bind
/// [2]: Binder::Runtime
/// [3]: RuntimeBinder::Target
/// [4]: Binder
///
/// # Stability
///
/// This trait is covered by the application-side API stability guarantee with
/// the exception of its members, which are implementation details.
pub trait FnBind<Binder> {
    type Output: 'static;
    type BoundFn: FnOnce() -> Self::Output + Copy + Send + 'static;

    fn bind(self, binder: Binder, ctx: &mut CfgBindCtx<'_>) -> Self::BoundFn;
}

macro_rules! impl_fn_bind {
    ( @start $($x:tt)* ) => {
        impl_fn_bind! { @iter [] [$($x)*] }
    };

    // inductive case
    ( @iter
        [$(($BinderI:ident, $RuntimeBinderI:ident, $fieldI:ident, $I:tt))*]
        [$next_head:tt $($next_tail:tt)*]
    ) => {
        impl_fn_bind! { @iter [$(($BinderI, $RuntimeBinderI, $fieldI, $I))* $next_head] [$($next_tail)*] }

        const _: () = {
            impl<
                T,
                Output,
                $( $BinderI, $RuntimeBinderI, )*
            > const FnBind<( $( $BinderI, )* )> for T
            where
                $( $BinderI: ~const Binder<Runtime = $RuntimeBinderI>, )*
                $( $RuntimeBinderI: RuntimeBinder, )*
                T: for<'call> FnOnce($( <$BinderI::Runtime as RuntimeBinder>::Target<'call>, )*)
                    -> Output + Copy + Send + 'static,
                Output: 'static,
            {
                type Output = Output;

                // This opaque type must be defined outside this trait to
                // prevent the unintended capturing of `$BinderI`.
                // [ref:opaque_type_extraneous_capture]
                type BoundFn = BoundFn<T, Output, $( $RuntimeBinderI, )*>;

                fn bind(
                    self,
                    binder: ( $( $BinderI, )* ),
                    ctx: &mut CfgBindCtx<'_>,
                ) -> Self::BoundFn {
                    Binder::register_dependency(&binder, ctx);

                    let intermediate = Binder::into_runtime_binder(binder);
                    bind_inner(self, intermediate)
                }
            }

            type BoundFn<T, Output, $( $RuntimeBinderI, )*>
            where
                $( $RuntimeBinderI: RuntimeBinder, )*
                T: for<'call> FnOnce($( $RuntimeBinderI::Target<'call>, )*)
                    -> Output + Copy + Send + 'static,
             = impl FnOnce() -> Output + Copy + Send + 'static;

            const fn bind_inner<
                T,
                Output,
                $( $RuntimeBinderI, )*
            >(
                func: T,
                runtime_binders: ( $( $RuntimeBinderI, )* ),
            ) -> BoundFn<T, Output, $( $RuntimeBinderI, )*>
            where
                $( $RuntimeBinderI: RuntimeBinder, )*
                T: for<'call> FnOnce($( $RuntimeBinderI::Target<'call>, )*)
                    -> Output + Copy + Send + 'static,
            {
                #[inline]
                move || {
                    // Safety: `runtime_binders` was created by the corresponding
                    // type's `into_runtime_binder` method.
                    // `CfgBindRegistry::finalize` checks that the borrowing
                    // rules regarding the materialization output are observed.
                    // If the check fails, so does the compilation, and this
                    // runtime code will never be executed.
                    let ($( $fieldI, )*) = unsafe {
                        <( $( $RuntimeBinderI, )* ) as RuntimeBinder>::materialize(runtime_binders)
                    };
                    func($( $fieldI, )*)
                }
            }
        }; // const _
    }; // end of macro arm

    // base case
    ( @iter [$($_discard:tt)*] [] ) => {}
}

seq_macro::seq!(I in 0..16 { impl_fn_bind! { @start #( (Binder~I, RuntimeBinder~I, field~I, I) )* } });

/// An uninhabited [`FnBind`] implementation.
///
/// # Stability
///
/// This trait is covered by the application-side API stability guarantee.
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct FnBindNever<T> {
    _phantom: PhantomData<T>,
    uninhabited: core::convert::Infallible,
}

impl<T: 'static, Binder> const FnBind<Binder> for FnBindNever<T>
where
    Binder: ~const self::Binder,
{
    type Output = T;
    type BoundFn = fn() -> T; // FIXME: Make this uninhabited

    fn bind(self, _: Binder, _: &mut CfgBindCtx<'_>) -> Self::BoundFn {
        match self.uninhabited {}
    }
}

/// Apply a function to an `impl `[`FnBind`][]`<_>`'s output.
///
/// # Example
///
/// ```rust
/// #![feature(const_trait_impl)]
/// #![feature(const_mut_refs)]
/// use core::cell::UnsafeCell;
/// use r3_core::{kernel::{cfg, traits}, bind::{Bind, FnBind, fn_bind_map}};
///
/// /// Like `r3::bind::bind` but wraps the output in `UnsafeCell`
/// const fn unsafe_cell_bind<'pool, C, Binder, Func, T>(
///     cfg: &mut cfg::Cfg<'pool, C>,
///     binder: Binder,
///     func: Func,
/// ) -> Bind<'pool, C::System, UnsafeCell<T>>
/// where
///     C: ~const traits::CfgBase,
///     C::System: traits::KernelBase + traits::KernelStatic,
///     Func: ~const FnBind<Binder, Output = T>,
///     T: 'static,
/// {
///     Bind::define()
///         // Apply `UnsafeCell::new` on `func`'s output
///         .init_with_bind(binder, fn_bind_map(func, UnsafeCell::new))
///         .finish(cfg)
/// }
/// ```
pub const fn fn_bind_map<FnBind, Mapper>(
    inner: FnBind,
    mapper: Mapper,
) -> FnBindMap<FnBind, Mapper> {
    FnBindMap { inner, mapper }
}

/// Applies a function to a [`FnBind`][]'s output.
///
/// Created by [`fn_bind_map`][].
pub struct FnBindMap<Inner, Mapper> {
    inner: Inner,
    mapper: Mapper,
}

impl<Binder, Inner, InnerBoundFn, Output, Mapper, NewOutput> const FnBind<Binder>
    for FnBindMap<Inner, Mapper>
where
    Inner: ~const FnBind<Binder, Output = Output, BoundFn = InnerBoundFn>,
    InnerBoundFn: FnOnce() -> Output + Copy + Send + 'static,
    Output: 'static,
    Mapper: FnOnce(Output) -> NewOutput + Copy + Send + 'static,
    NewOutput: 'static,
{
    type Output = NewOutput;

    // This opaque type must be defined outside this trait to
    // prevent the unintended capturing of `Binder`.
    // [ref:opaque_type_extraneous_capture]
    type BoundFn = MappedBoundFn<InnerBoundFn, Output, Mapper, NewOutput>;

    fn bind(self, binder: Binder, ctx: &mut CfgBindCtx<'_>) -> Self::BoundFn {
        map_bind_inner(self.inner.bind(binder, ctx), self.mapper)
    }
}

type MappedBoundFn<InnerBoundFn, Output, Mapper, NewOutput>
where
    InnerBoundFn: FnOnce() -> Output + Copy + Send + 'static,
    Mapper: FnOnce(Output) -> NewOutput + Copy + Send + 'static,
= impl FnOnce() -> NewOutput + Copy + Send + 'static;

const fn map_bind_inner<InnerBoundFn, Output, Mapper, NewOutput>(
    inner_bound_fn: InnerBoundFn,
    mapper: Mapper,
) -> MappedBoundFn<InnerBoundFn, Output, Mapper, NewOutput>
where
    InnerBoundFn: FnOnce() -> Output + Copy + Send + 'static,
    Mapper: FnOnce(Output) -> NewOutput + Copy + Send + 'static,
{
    move || (mapper)(inner_bound_fn())
}

// Binder traits
// ----------------------------------------------------------------------------

/// Represents a *binder*, which represents a specific way to access the
/// contents of a [binding][1] from a runtime function.
///
/// See [the module-level documentation][2] for more.
///
/// # Stability
///
/// This trait is covered by the application-side API stability guarantee with
/// a few exceptions, which are documented on a per-item basis.
///
/// [1]: Bind
/// [2]: index.html#binders
pub trait Binder {
    /// The runtime representation of `Self`.
    ///
    /// # Stability
    ///
    /// This method is unstable.
    type Runtime: RuntimeBinder;

    /// Define a binding dependency in `CfgBindCtx::bind_registry`.
    ///
    /// # Stability
    ///
    /// This method is unstable.
    fn register_dependency(&self, ctx: &mut CfgBindCtx<'_>);

    /// Convert `self` to the runtime representation ([`RuntimeBinder`][]).
    ///
    /// # Stability
    ///
    /// This method is unstable.
    fn into_runtime_binder(self) -> Self::Runtime;
}

/// Unstable. The runtime representation of [`Binder`][].
///
/// This trait signifies the following properties regarding an implementing
/// type:
///
///  - `self` can be "materialized" as `Self::Target<'call>` at runtime.
///
///  - `'call` represets the duration during which `Self::Target` is used. If
///    `Self::Target<'call>` is a reference, its lifetime parameter may be
///    bound to `'call`. Some binder types don't require this.
///
/// # Stability
///
/// This trait is unstable.
pub trait RuntimeBinder: Send + Copy + 'static {
    /// The materialized form.
    type Target<'call>;

    /// Construct a target object at runtime, using the intermediate product
    /// constructed by [`Binder::into_runtime_binder`].
    ///
    /// # Safety
    ///
    /// `intermediate` must have been constructed by
    /// `<Self as Binder>::into_runtime_binder`.
    ///
    /// The caller must uphold that `Self::Target` is safe to exist. (The
    /// configuration system is reponsible for enforcing this property.)
    unsafe fn materialize<'call>(self) -> Self::Target<'call>;
}

/// Unstable. The runtime representation of [`BindTake`][].
///
/// # Stability
///
/// This trait is unstable.
pub struct RuntimeBindTake<System, T>(BindHunk<System, T>);

impl<System, T> Clone for RuntimeBindTake<System, T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}
impl<System, T> Copy for RuntimeBindTake<System, T> {}

/// Materializes `BindTake<System, T>` as `T`.
impl<T, System> const Binder for BindTake<'_, System, T>
where
    T: 'static,
    System: raw::KernelBase + cfg::KernelStatic,
{
    type Runtime = RuntimeBindTake<System, T>;

    fn register_dependency(&self, ctx: &mut CfgBindCtx<'_>) {
        if matches!(ctx.usage, BindUsage::Executable) {
            panic!(
                "an executable object can not consume `BindTake` because the \
                executable object may run for multiple times, but the binding \
                value can be moved out only once"
            );
        }

        let Bind {
            bind_registry,
            bind_i,
            ..
        } = self.0;
        bind_registry.borrow_mut().binds[bind_i]
            .users
            .push((ctx.usage, BindBorrowType::Take));
    }

    fn into_runtime_binder(self) -> Self::Runtime {
        RuntimeBindTake(self.0.hunk)
    }
}

impl<T, System> RuntimeBinder for RuntimeBindTake<System, T>
where
    T: 'static,
    System: raw::KernelBase + cfg::KernelStatic,
{
    type Target<'call> = T;

    #[inline]
    unsafe fn materialize<'call>(self) -> Self::Target<'call> {
        unsafe { BindHunk::as_ref(self.0).0.get().read().assume_init() }
    }
}

/// Unstable. The runtime representation of [`BindTakeMut`][].
///
/// # Stability
///
/// This trait is unstable.
pub struct RuntimeBindTakeMut<System, T>(BindHunk<System, T>);

impl<System, T> Clone for RuntimeBindTakeMut<System, T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}
impl<System, T> Copy for RuntimeBindTakeMut<System, T> {}

/// Materializes `BindTakeMut<System, T>` as `&'static mut T`.
impl<T, System> const Binder for BindTakeMut<'_, System, T>
where
    T: 'static,
    System: raw::KernelBase + cfg::KernelStatic,
{
    type Runtime = RuntimeBindTakeMut<System, T>;

    fn register_dependency(&self, ctx: &mut CfgBindCtx<'_>) {
        if matches!(ctx.usage, BindUsage::Executable) {
            panic!(
                "an executable object can not consume `BindTakeMut` because the \
                executable object may run for multiple times, but multiple \
                mutable borrows of the binding are not allowed to exist \
                simultaneously"
            );
        }

        let Bind {
            bind_registry,
            bind_i,
            ..
        } = self.0;
        bind_registry.borrow_mut().binds[bind_i]
            .users
            .push((ctx.usage, BindBorrowType::TakeMut));
    }

    fn into_runtime_binder(self) -> Self::Runtime {
        RuntimeBindTakeMut(self.0.hunk)
    }
}

impl<T, System> RuntimeBinder for RuntimeBindTakeMut<System, T>
where
    T: 'static,
    System: raw::KernelBase + cfg::KernelStatic,
{
    type Target<'call> = &'static mut T;

    #[inline]
    unsafe fn materialize<'call>(self) -> Self::Target<'call> {
        unsafe { BindHunk::as_ref(self.0).assume_init_mut() }
    }
}

/// Unstable. The runtime representation of [`BindTakeRef`][].
///
/// # Stability
///
/// This trait is unstable.
pub struct RuntimeBindTakeRef<System, T>(BindHunk<System, T>);

impl<System, T> Clone for RuntimeBindTakeRef<System, T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}
impl<System, T> Copy for RuntimeBindTakeRef<System, T> {}

/// Materializes `BindTakeRef<System, T>` as `&'static T`.
impl<T, System> const Binder for BindTakeRef<'_, System, T>
where
    T: 'static + Sync,
    System: raw::KernelBase + cfg::KernelStatic,
{
    type Runtime = RuntimeBindTakeRef<System, T>;

    fn register_dependency(&self, ctx: &mut CfgBindCtx<'_>) {
        let Bind {
            bind_registry,
            bind_i,
            ..
        } = self.0;
        bind_registry.borrow_mut().binds[bind_i]
            .users
            .push((ctx.usage, BindBorrowType::TakeRef));
    }

    fn into_runtime_binder(self) -> Self::Runtime {
        RuntimeBindTakeRef(self.0.hunk)
    }
}

impl<T, System> RuntimeBinder for RuntimeBindTakeRef<System, T>
where
    T: 'static,
    System: raw::KernelBase + cfg::KernelStatic,
{
    type Target<'call> = &'static T;

    #[inline]
    unsafe fn materialize<'call>(self) -> Self::Target<'call> {
        unsafe { BindHunk::as_ref(self.0).assume_init_ref() }
    }
}

/// Unstable. The runtime representation of [`BindBorrow`][].
///
/// # Stability
///
/// This trait is unstable.
pub struct RuntimeBindBorrow<System, T>(BindHunk<System, T>);

impl<System, T> Clone for RuntimeBindBorrow<System, T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}
impl<System, T> Copy for RuntimeBindBorrow<System, T> {}

/// Materializes `BindBorrow<System, T>` as `&'call T`.
impl<T, System> const Binder for BindBorrow<'_, System, T>
where
    T: 'static,
    System: raw::KernelBase + cfg::KernelStatic,
{
    type Runtime = RuntimeBindBorrow<System, T>;

    fn register_dependency(&self, ctx: &mut CfgBindCtx<'_>) {
        let Bind {
            bind_registry,
            bind_i,
            ..
        } = self.0;

        let borrow_type = match ctx.usage {
            BindUsage::Bind(_) => BindBorrowType::Borrow,
            BindUsage::Executable => BindBorrowType::TakeRef,
        };

        bind_registry.borrow_mut().binds[bind_i]
            .users
            .push((ctx.usage, borrow_type));
    }

    fn into_runtime_binder(self) -> Self::Runtime {
        RuntimeBindBorrow(self.0.hunk)
    }
}

impl<T, System> RuntimeBinder for RuntimeBindBorrow<System, T>
where
    T: 'static,
    System: raw::KernelBase + cfg::KernelStatic,
{
    type Target<'call> = &'call T;

    #[inline]
    unsafe fn materialize<'call>(self) -> Self::Target<'call> {
        unsafe { BindHunk::as_ref(self.0).assume_init_ref() }
    }
}

/// Unstable. The runtime representation of [`BindBorrowMut`][].
///
/// # Stability
///
/// This trait is unstable.
pub struct RuntimeBindBorrowMut<System, T>(BindHunk<System, T>);

impl<System, T> Clone for RuntimeBindBorrowMut<System, T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}
impl<System, T> Copy for RuntimeBindBorrowMut<System, T> {}

/// Materializes `BindBorrowMut<System, T>` as `&'call mut T`.
impl<T, System> const Binder for BindBorrowMut<'_, System, T>
where
    T: 'static,
    System: raw::KernelBase + cfg::KernelStatic,
{
    type Runtime = RuntimeBindBorrowMut<System, T>;

    fn register_dependency(&self, ctx: &mut CfgBindCtx<'_>) {
        let Bind {
            bind_registry,
            bind_i,
            ..
        } = self.0;

        let borrow_type = match ctx.usage {
            BindUsage::Bind(_) => BindBorrowType::BorrowMut,
            BindUsage::Executable => BindBorrowType::TakeMut,
        };

        bind_registry.borrow_mut().binds[bind_i]
            .users
            .push((ctx.usage, borrow_type));
    }

    fn into_runtime_binder(self) -> Self::Runtime {
        RuntimeBindBorrowMut(self.0.hunk)
    }
}

impl<T, System> RuntimeBinder for RuntimeBindBorrowMut<System, T>
where
    T: 'static,
    System: raw::KernelBase + cfg::KernelStatic,
{
    type Target<'call> = &'call mut T;

    #[inline]
    unsafe fn materialize<'call>(self) -> Self::Target<'call> {
        unsafe { BindHunk::as_ref(self.0).assume_init_mut() }
    }
}

/// Materializes `BindRef<System, T>` as `&'static T`. Can only be consumed by
/// executables and not by bindings.
impl<T, System> const Binder for BindRef<System, T>
where
    T: 'static,
    System: raw::KernelBase + cfg::KernelStatic,
{
    type Runtime = Self;

    fn register_dependency(&self, ctx: &mut CfgBindCtx<'_>) {
        match ctx.usage {
            BindUsage::Executable => {
                // Already registered by the call to `Bind::as_ref` that created
                // `*self`
            }
            BindUsage::Bind(_) => {
                // `BindTable` is only safely available for executables
                panic!(
                    "`BindRef` can not be consumed by a binding producer; \
                    consider using `BindTakeRef` instead"
                );
            }
        }
    }

    fn into_runtime_binder(self) -> Self::Runtime {
        self
    }
}

impl<T, System> RuntimeBinder for BindRef<System, T>
where
    T: 'static,
    System: raw::KernelBase + cfg::KernelStatic,
{
    type Target<'call> = &'static T;

    #[inline]
    unsafe fn materialize<'call>(self) -> Self::Target<'call> {
        unsafe { &BindTable::get_unchecked()[self] }
    }
}

macro_rules! impl_binder_on_tuples {
    ( @start $($x:tt)* ) => {
        impl_binder_on_tuples! { @iter [] [$($x)*] }
    };

    // inductive case
    ( @iter
        [$(($BinderI:ident, $RuntimeBinderI:ident, $I:tt))*]
        [$next_head:tt $($next_tail:tt)*]
    ) => {
        impl_binder_on_tuples! { @iter [$(($BinderI, $RuntimeBinderI, $I))* $next_head] [$($next_tail)*] }

        impl<$( $BinderI, )*> const Binder for ($( $BinderI, )*)
        where
            $( $BinderI: ~const Binder, )*
        {
            type Runtime = ( $( $BinderI::Runtime, )* );

            fn register_dependency(&self, ctx: &mut CfgBindCtx<'_>) {
                $( self.$I.register_dependency(ctx); )*
                let _ = ctx;
            }

            #[allow(clippy::unused_unit)]
            fn into_runtime_binder(self) -> Self::Runtime {
                ( $( self.$I.into_runtime_binder(), )* )
            }
        }

        impl<$( $RuntimeBinderI, )*> RuntimeBinder for ($( $RuntimeBinderI, )*)
        where
            $( $RuntimeBinderI: RuntimeBinder, )*
        {
            type Target<'call> = ( $( $RuntimeBinderI::Target<'call>, )* );

            #[allow(unused_unsafe)]
            #[allow(unused_variables)]
            #[allow(clippy::unused_unit)]
            #[inline]
            unsafe fn materialize<'call>(self) -> Self::Target<'call> {
                unsafe {
                    ( $( $RuntimeBinderI::materialize(self.$I), )* )
                }
            }
        }
    };

    // base case
    ( @iter [$($_discard:tt)*] [] ) => {}
}

seq_macro::seq!(I in 0..16 {
    impl_binder_on_tuples! { @start #( (Binder~I, RuntimeBinder~I, I) )* }
});

impl<const LEN: usize, Binder> const self::Binder for [Binder; LEN]
where
    Binder: ~const self::Binder,
{
    type Runtime = [Binder::Runtime; LEN];

    fn register_dependency(&self, ctx: &mut CfgBindCtx<'_>) {
        // `for` loops are unusable in `const fn` [ref:const_for]
        let mut i = 0;
        while i < LEN {
            self[i].register_dependency(ctx);
            i += 1;
        }
    }

    fn into_runtime_binder(self) -> Self::Runtime {
        unsafe {
            // `[T; N]::map` is unusable in `const fn` [ref:const_array_map]
            let mut out: [MaybeUninit<Binder::Runtime>; LEN] = MaybeUninit::uninit_array();
            let this = MaybeUninit::new(self);
            // `for` loops are unusable in `const fn` [ref:const_for]
            let mut i = 0;
            while i < LEN {
                out[i] = MaybeUninit::new(
                    this.as_ptr()
                        .cast::<Binder>()
                        .wrapping_add(i)
                        .read()
                        .into_runtime_binder(),
                );
                i += 1;
            }
            // Safety: All elements of `out` are initialized
            MaybeUninit::array_assume_init(out)
        }
    }
}

impl<const LEN: usize, RuntimeBinder> self::RuntimeBinder for [RuntimeBinder; LEN]
where
    RuntimeBinder: self::RuntimeBinder,
{
    type Target<'call> = [RuntimeBinder::Target<'call>; LEN];

    #[inline]
    unsafe fn materialize<'call>(self) -> Self::Target<'call> {
        // Safety: Upheld by the caller
        self.map(|x| unsafe { RuntimeBinder::materialize(x) })
    }
}

// ----------------------------------------------------------------------------
