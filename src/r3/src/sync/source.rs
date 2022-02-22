//! Abstracts the various ways to provide an initial value to static cell types
//! (e.g., [`Mutex`][1]). It's mostly used through their definer types.
//!
//! [1]: crate::sync::mutex::StaticMutex
use core::{cell::UnsafeCell, marker::PhantomData, mem::MaybeUninit};

use crate::bind::{bind, bind_default, Bind, FnBind};
use r3_core::{
    hunk::Hunk,
    kernel::{cfg::Cfg, traits},
    utils::Init,
};

/// Abstracts the various ways to provide an initial value to static cell types
/// (e.g., [`Mutex`][1]). It's mostly used through their definer types.
///
/// # Stability
///
/// This trait is unstable.
///
/// [1]: crate::sync::mutex::StaticMutex
pub trait Source<System> {
    type Target: 'static;

    /// Construct a `Hunk` to store the value.
    ///
    /// `Self::Target` in the returned `Hunk` is safe to access from executable
    /// objects through `MaybeUninit`. The client may grant mutable access to
    /// `Self::Target` through `UnsafeCell` while observing the borrow rules.
    /// <!-- [tag:source_cell] -->
    fn into_unsafe_cell_hunk<C>(
        self,
        cfg: &mut Cfg<C>,
    ) -> Hunk<System, UnsafeCell<MaybeUninit<Self::Target>>>
    where
        C: ~const traits::CfgBase<System = System>;
}

/// A [`Source`][] that provides a hunk initialized by
/// [`Default::default`][].
pub struct DefaultSource<T>(PhantomData<T>);

impl<T> Init for DefaultSource<T> {
    const INIT: Self = Self(PhantomData);
}

impl<System, T> const Source<System> for DefaultSource<T>
where
    System: traits::KernelBase + traits::KernelStatic,
    T: 'static + Default + Send,
{
    type Target = T;

    fn into_unsafe_cell_hunk<C>(
        self,
        cfg: &mut Cfg<C>,
    ) -> Hunk<System, UnsafeCell<MaybeUninit<Self::Target>>>
    where
        C: ~const traits::CfgBase<System = System>,
    {
        TakeBindSource(bind_default(cfg)).into_unsafe_cell_hunk(cfg)
    }
}

/// A [`Source`][] that provides a hunk initialized by a user-provided
/// initializer.
pub struct NewBindSource<Binder, Func> {
    pub binder: Binder,
    pub func: Func,
}

impl<System, Binder, Func> const Source<System> for NewBindSource<Binder, Func>
where
    System: traits::KernelBase + traits::KernelStatic,
    Func: ~const FnBind<Binder>,
    Func::Output: Send,
{
    type Target = Func::Output;

    fn into_unsafe_cell_hunk<C>(
        self,
        cfg: &mut Cfg<C>,
    ) -> Hunk<System, UnsafeCell<MaybeUninit<Self::Target>>>
    where
        C: ~const traits::CfgBase<System = System>,
    {
        let Self { binder, func } = self;
        TakeBindSource(bind(binder, func).finish(cfg)).into_unsafe_cell_hunk(cfg)
    }
}

/// A [`Source`][] that consumes a user-provided binding.
pub struct TakeBindSource<'pool, System, T>(pub(crate) Bind<'pool, System, T>);

impl<System, T> const Source<System> for TakeBindSource<'_, System, T>
where
    System: traits::KernelBase + traits::KernelStatic,
    T: 'static + Send,
{
    type Target = T;

    fn into_unsafe_cell_hunk<C>(
        self,
        cfg: &mut Cfg<C>,
    ) -> Hunk<System, UnsafeCell<MaybeUninit<Self::Target>>>
    where
        C: ~const traits::CfgBase<System = System>,
    {
        // Dummy depenedency to keep the binding alive
        bind((self.0.take_mut(),), |_: &mut _| {})
            .unpure()
            .finish(cfg);

        // Take the raw `Hunk<_, BindData<T>>` and transmute it into `Hunk<_,
        // UnsafeCell<MaybeUninit<T>>>`
        // Safety: The dummy dependency defined above ensures exclusive access
        // to the hunk. `UnsafeCell<MaybeUninit<T>>` has a compatible
        // representation with `BindData<T>`.
        unsafe { self.0.hunk().transmute::<UnsafeCell<MaybeUninit<T>>>() }
    }
}

/// A [`Source`][] that provides a user-provided hunk, assuming the provider
/// upholds the safety rules.
pub struct HunkSource<System, T>(pub(crate) Hunk<System, UnsafeCell<MaybeUninit<T>>>);

impl<System, T> const Source<System> for HunkSource<System, T>
where
    T: 'static,
{
    type Target = T;

    fn into_unsafe_cell_hunk<C>(
        self,
        _: &mut Cfg<C>,
    ) -> Hunk<System, UnsafeCell<MaybeUninit<Self::Target>>>
    where
        C: ~const traits::CfgBase<System = System>,
    {
        self.0
    }
}

#[macropol::macropol] // Replace `$metavariables` in literals and doc comments
macro_rules! impl_source_setter {
    (
        $( #[$($meta:tt)*] )*
        impl $DefinerName:ident<$($Param:tt)*
    ) => {
        impl_source_setter! {
            @iter
            attributes: {{ $( #[$($meta)*] )* }},
            definer_name: {{ $DefinerName }},
            generics: {{ }},
            definer_ty: {{ $DefinerName< }},
            param_template: {{ $($Param)* }},
            concrete_source_var: {{ $ConcreteSource }},
        }
    };

    // Preprocessing
    // -------------------------------------------------------------------
    //
    // These rules consume `param_template` one token tree by one and produce
    // `generics` and `definer_ty`.
    (@iter
        attributes: {$attributes:tt},
        definer_name: {$definer_name:tt},
        generics: {$generics:tt},
        definer_ty: {{ $( $definer_ty:tt )* }},
        param_template: {{> $($rest:tt)* }},
        concrete_source_var: {$concrete_source_var:tt},
    ) => {
        $(
            compile_error!(concat!("Extra token in input: `$&rest`"));
        )*

        impl_source_setter! {
            @end
            attributes: {$attributes},
            definer_name: {$definer_name},
            generics: {$generics},
            definer_ty: {{ $( $definer_ty )* > }},
            concrete_source_var: {$concrete_source_var},
        }
    };

    (@iter
        attributes: {$attributes:tt},
        definer_name: {$definer_name:tt},
        generics: {{ $( $generics:tt )* }},
        definer_ty: {{ $( $definer_ty:tt )* }},
        param_template: {{ #Source $($rest:tt)* }},
        concrete_source_var: {{ $( $concrete_source_var:tt )* }},
    ) => {
        impl_source_setter! {
            @iter
            attributes: {$attributes},
            definer_name: {$definer_name},
            generics: {{ $( $generics )* T }},
            definer_ty: {{ $( $definer_ty )* $( $concrete_source_var )* }},
            param_template: {{ $($rest)* }},
            concrete_source_var: {{ $( $concrete_source_var )* }},
        }
    };

    (@iter
        attributes: {$attributes:tt},
        definer_name: {$definer_name:tt},
        generics: {{ $( $generics:tt )* }},
        definer_ty: {{ $( $definer_ty:tt )* }},
        param_template: {{ $any:tt $($rest:tt)* }},
        concrete_source_var: { $concrete_source_var:tt },
    ) => {
        impl_source_setter! {
            @iter
            attributes: {$attributes},
            definer_name: {$definer_name},
            generics: {{ $( $generics )* $any }},
            definer_ty: {{ $( $definer_ty )* $any }},
            param_template: {{ $($rest)* }},
            concrete_source_var: { $concrete_source_var },
        }
    };

    // Output
    // -------------------------------------------------------------------
    (@end
        attributes: {{
            // When this attribute is present, `init[_bind]` will automatically
            // wrap the output with the specified wrapper type.
            $( #[autowrap($wrap_with:expr, $Wrapper:ident)] )?
            // The opposite of `#[autowrap]`. `$no_autowrap` == `()`
            $( #[no_autowrap$no_autowrap:tt] )?
        }},
        definer_name: {{ $definer_name:ident }},
        // Generic parameters. `#Source` is replaced with `T` here.
        generics: {{ $( $generics:tt )* }},
        // A template for a concrete definer type.
        // `#Source` is replaced with `$ConcreteSource` here.
        definer_ty: {{ $( $definer_ty:tt )* }},
        concrete_source_var: {{ $( $concrete_source_var:tt )* }},
    ) => {
        // The anonymous `const` trick to introduce scoping breaks
        // documentation, unfortunately.
        mod __pr {
            pub use crate::{
                sync::source::{Source, NewBindSource, TakeBindSource, HunkSource},
                bind::{Bind, fn_bind_map, FnBindMap},
                hunk::Hunk,
            };
            pub use core::{cell::UnsafeCell, mem::MaybeUninit};
        }

        macro_rules! Definer {
            ( $( $concrete_source_var )*:ty ) => { $( $definer_ty )* };
        }

        // `#[autowrap]`
        $(
            // FIXME: False `type_alias_bounds`; the `'static` bound is required
            #[allow(type_alias_bounds)]
            type MappedFunc<Func, T: 'static> = __pr::FnBindMap<
                Func,
                impl FnOnce(T) -> $Wrapper<T> + Send + Copy + 'static
            >;
            type AutoWrapped<T> = $Wrapper<T>;
            macro MappedFunc($Func:ty, $T:ty) { MappedFunc<$Func, $T> }
        )?

        // `#[no_autowrap]`
        $(
            type AutoWrapped<T> = T;
            /// $&no_autowrap
            macro MappedFunc($Func:ty, $T:ty) { $Func }
        )?

        /// # Initial Value
        ///
        /// The following methods specify the initial value for the contained
        /// object (`T`).
        ///
        /// If none of the following methods are called, `T` will be constructed
        /// by its [`Default`][] implementation.
        // [tag:default_source_is_default] `DefaultSource<T>` is the default, so
        // the following `impl` block allows up to only one modification to
        // `Self::source`.
        impl<$( $generics )*> Definer!(DefaultSource<AutoWrapped<T>>)
        where
            T: 'static
        {
            /// Use the specified function to provide the initial value.
            ///
            /// The semantics of the method's parameter is similar to that of
            /// [`BindDefiner::init`][1].
            ///
            /// [1]: crate::bind::BindDefiner::init
            pub const fn init<Func>(self, func: Func)
                -> Definer!(__pr::NewBindSource<(), MappedFunc!(Func, T)>)
            where
                // Demand the unchangedness of `Source::Target` so that the
                // initial creation of `Self` doesn't have an unconstrained `T`
                __pr::NewBindSource<(), MappedFunc!(Func, T)>:
                    __pr::Source<System, Target = AutoWrapped<T>>,
            {
                $definer_name {
                    source: __pr::NewBindSource {
                        binder: (),
                        func $( : __pr::fn_bind_map(func, $wrap_with) )?,
                    },
                    ..self
                }
            }

            /// Use the specified function with dependency to provide the
            /// initial value.
            ///
            /// The semantics of the method's parameters is similar to that of
            /// [`BindDefiner::init_with_bind`][1].
            ///
            /// [1]: crate::bind::BindDefiner::init_with_bind
            pub const fn init_with_bind<Binder, Func>(
                self,
                binder: Binder,
                func: Func,
            ) -> Definer!(__pr::NewBindSource<Binder, MappedFunc!(Func, T)>)
            where
                // Demand the unchangedness of `Source::Target` so that the
                // initial creation of `Self` doesn't have an unconstrained `T`
                __pr::NewBindSource<Binder, MappedFunc!(Func, T)>:
                    __pr::Source<System, Target = AutoWrapped<T>>,
            {
                $definer_name {
                    source: __pr::NewBindSource {
                        binder,
                        func $( : __pr::fn_bind_map(func, $wrap_with) )?,
                    },
                    ..self
                }
            }

            /// [Take] the specified binding to use as this object's contents.
            ///
            $(
            /// <div class="admonition-follows"></div>
            ///
            /// > **Warning:** Unlike [`mutex::Definer::take_bind`][1], this
            /// > method expects that `T` is already wrapped by `$&Wrapper` in
            /// > the specified binding.
            ///
            /// [1]: crate::sync::mutex::Definer::take_bind
            /// [2]: crate::bind::Bind::transmute
            )?
            ///
            /// [Take]: crate::bind::Bind::take_mut
            pub const fn take_bind<'pool>(
                self,
                bind: __pr::Bind<'pool, System, AutoWrapped<T>>,
            ) -> Definer!(__pr::TakeBindSource<'pool, System, AutoWrapped<T>>) {
                $definer_name {
                    source: __pr::TakeBindSource(bind),
                    ..self
                }
            }

            /// Use the specified existing hunk as this object's storage.
            ///
            $(
            /// <div class="admonition-follows"></div>
            ///
            /// > **Warning:**
            /// > Unlike [`mutex::Definer::wrap_hunk_unchecked`][1], this
            /// > method expects that `T` is already wrapped by `$&Wrapper` in
            /// > the specified binding. *A care must be taken not to
            /// > accidentally [transmute][2] `T` into `$&Wrapper<T>`* when
            /// > passing a hunk that has the same representation as `T` but
            /// > requires transmutation due to having a different set of
            /// > wrappers, such as `MaybeUninit` and `UnsafeCell`.
            ///
            /// [1]: crate::sync::mutex::Definer::take_bind
            /// [2]: crate::hunk::Hunk::transmute
            )?
            ///
            /// # Safety
            ///
            /// This object grants exclusive access (`&mut T`) to the
            /// `UnsafeCell`'s contents based on the assumption that no accesses
            /// are made through other means, and that `T` is initialized by
            /// the time the boot phase completes.
            pub const unsafe fn wrap_hunk_unchecked<'pool>(
                self,
                hunk: __pr::Hunk<System, __pr::UnsafeCell<__pr::MaybeUninit<AutoWrapped<T>>>>,
            ) -> Definer!(__pr::HunkSource<System, AutoWrapped<T>>)
            {
                $definer_name {
                    source: __pr::HunkSource(hunk),
                    ..self
                }
            }
        }
    };
}
