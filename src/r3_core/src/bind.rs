//! Bindings
use core::{cell::UnsafeCell, mem::MaybeUninit};

use crate::{
    closure::Closure,
    hunk::Hunk,
    kernel::{self, cfg, raw, raw_cfg, StartupHook},
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

    // FIXME: `clone_from` is not `#[default_method_body_is_const]` yet
    #[inline]
    fn clone_from(&mut self, source: &Self) {
        *self = *source;
    }
}

/// A [binder](Binder) that gives `&T` to a bound function.
///
/// Created by [`Bind::borrow`][].
pub struct BindBorrow<'pool, System, T>(Bind<'pool, System, T>);

/// A [binder](Binder) that gives `&mut T` to a bound function.
///
/// Created by [`Bind::borrow_mut`][].
pub struct BindBorrowMut<'pool, System, T>(Bind<'pool, System, T>);

/// A [binder](Binder) that gives `T` to a bound function.
///
/// Created by [`Bind::take`][].
pub struct BindTake<'pool, System, T>(Bind<'pool, System, T>);

/// A [binder](Binder) that gives `&'static T` to a bound function.
///
/// Created by [`Bind::take_ref`][].
pub struct BindTakeRef<'pool, System, T>(Bind<'pool, System, T>);

/// A [binder](Binder) that gives `&'static mut T` to a bound function.
///
/// Created by [`Bind::take_mut`][].
pub struct BindTakeMut<'pool, System, T>(Bind<'pool, System, T>);

/// A reference to a [binding][1]. Created by [`Bind::as_ref`][].
///
/// It doesn't provide access to the contents by itself because it could be
/// used before the binding is initialized. Index [`BindTable`][] by this type to
/// borrow the contents as `&'static T`.
///
/// [1]: Bind
pub struct BindRef<System, T>(BindHunk<System, T>);

impl<System, T> Copy for BindRef<System, T> {}
impl<System, T> const Clone for BindRef<System, T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }

    // FIXME: `clone_from` is not `#[default_method_body_is_const]` yet
    #[inline]
    fn clone_from(&mut self, source: &Self) {
        *self = *source;
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
        T: Sync,
    {
        BindTakeMut(*self)
    }

    /// Construct a [`BindRef`][], which can be used to get `&'static T` from a
    /// [`BindTable`][]`<System>`.
    ///
    /// `BindRef` doubles as a binder that gives `&'static T` in a bound
    /// [executable object][1].
    ///
    /// The configuration system can't track the usages of `BindRef` (note the
    /// lack of a lifetime parameter in its definition). As such, merely calling
    /// this method counts as a use of the binding whether the returned
    /// `BindRef` is actually used or not.
    ///
    /// [1]: ExecutableDefiner
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

/// The definer (static builder) for [`Bind`].
#[must_use = "must call `finish()` to complete registration"]
pub struct BindDefiner<System, Binder, Func> {
    _phantom: PhantomInvariant<System>,
    binder: Binder,
    func: Func,
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
            func: private_bind_definer::FuncUnspecified,
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
            func,
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
            func,
            binder,
            ..self
        }
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
        <Func as FnBind<Binder>>::Output: 'static,
        <Func as FnBind<Binder>>::BoundFn: 'static,
    {
        let hunk = BindHunk::define().zeroed().finish(cfg);

        let bind_registry = &cfg.shared.bind_registry;
        let bind_i = bind_registry.borrow().binds.len();

        // Construct the initializer for the binding being created
        let mut ctx = CfgBindCtx {
            _phantom: &(),
            usage: BindUsage::Bind(bind_i),
        };
        let initializer = self.func.bind(self.binder, &mut ctx);
        let initializer = Closure::from_fn_const(move || {
            let output = initializer();
            // Safety: There's no conflicting borrows
            unsafe { hunk.0.get().write(MaybeUninit::new(output)) };
        });

        {
            let mut bind_registry = bind_registry.borrow_mut();
            assert!(bind_i == bind_registry.binds.len());
            let allocator = bind_registry.binds.allocator().clone();
            bind_registry.binds.push(CfgBindInfo {
                initializer,
                users: ComptimeVec::new_in(allocator),
            });
        }

        Bind {
            hunk,
            bind_registry,
            bind_i,
        }
    }
}

// TODO: Implement `UnzipBind` on `Bind<'_, System, (T0, T1)>`, etc.
/// A trait for breaking [`Bind`] into smaller parts.
pub trait UnzipBind<Target> {
    /// Break [`Bind`] into smaller parts.
    fn unzip(self) -> Target;
}

// Runtime binding registry
// ----------------------------------------------------------------------------

/// Represents a permission to dereference [`BindRef`][].
pub struct BindTable<System> {
    _phantom: PhantomInvariant<System>,
}

impl<System> BindTable<System>
where
    System: raw::KernelBase + cfg::KernelStatic,
{
    // TODO: pub fn get() -> Result<Self> {}

    /// Get a reference to `BindTable` without checking if it's safe to do so
    /// in the current context.
    ///
    /// # Safety
    ///
    /// The returned reference may be used to borrow binding contents that are
    /// uninitialized or being mutably borrowed somewhere else.
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

impl const Drop for CfgBindRegistry {
    fn drop(&mut self) {
        // FIXME: `ComptimeVec::drop` can't do this currently because of
        //        [ref:fixme_comptime_drop_elem]
        self.binds.clear();
    }
}

struct CfgBindInfo {
    /// The initializer for the binder. It'll be registered as a startup hook
    /// on finalization.
    initializer: Closure,
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
                    sorter::SorterError::BindCycle { bind_is: _ } => {
                        panic!("the binding initialization order contains a cycle");
                    }
                    sorter::SorterError::ConflictingIndefiniteBorrow { bind_i: _ } => {
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

        // FIXME: `for` loops are barely useful in `const fn` at the moment
        let mut i = 0;
        while i < callback.bind_init_order.len() {
            let bind_i = callback.bind_init_order[i];

            StartupHook::define()
                .start(self.binds[bind_i].initializer)
                .priority(INIT_HOOK_PRIORITY)
                .finish(cfg);

            i += 1;
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
///
/// TODO: For executables this could be reduced to `'static`
#[derive(Clone, Copy, PartialEq, Eq)]
enum BindBorrowType {
    /// Gives `&T` that is valid for the consumption duration.
    /// Invalid for executables [ref:borrow_is_indefinite_for_executable].
    Borrow,
    /// Gives `&mut T` that is valid for the consumption duration.
    /// Invalid for executables [ref:borrow_is_indefinite_for_executable].
    BorrowMut,
    /// Gives `T`. This is similar to [`Self::TakeMut`][] except that the
    /// storage may be freed up after the use.
    ///
    /// TODO: Implement the storage reuse
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
/// # Safety
///
/// At any point of time, the provided [`Closure`] must never be invoked by two
/// threads simultaneously. It can be called for multiple times, however.
pub unsafe trait ExecutableDefiner: Sized + private::Sealed {
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

/// A trait for closures that can receive materialized [bindings][1].
///
/// `FnBind<(B0, B1, ...)>` is implemented for `impl for<'call>
/// FnOnce(M0<'call>, M1<'call>, ...) + Copy + Send + 'static`, where `Mn<'call>
/// == Bn::`[`Runtime`][2]`::`[`Target`][3]`<'call>` (`Bn`'s materialized form).
///
/// [1]: Bind
/// [2]: Binder::Runtime
/// [3]: RuntimeBinder::Target
///
/// # Stability
///
/// This trait is covered by the application-side API stability guarantee with
/// the exception of its members, which are implementation details.
pub trait FnBind<Binder> {
    type Output;
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

                // FIXME: `impl` type alias in trait impls implicitly captures
                // the surrounding environment's generic parameters? That's
                // probably why the type alias has to be outside this `impl`
                // block, which has `$BinderI` and the compiler would demand the
                // removal of `+ 'static`.
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
                T: Copy + Send + 'static,
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

// Binder traits
// ----------------------------------------------------------------------------

/// Represents a *binder*, which represents a specific way to access the
/// contents of a [binding][1] from a runtime function.
///
/// There are methods that take binders and bind them to compatible closures
/// ([`FnBind`][]), namely [`ExecutableDefinerExt::start_with_bind`][] and
/// [`BindDefiner::init_with_bind`][]. When called, these methods wrap the given
/// closure with an outer closure, which "materializes" the provided binders at
/// runtime and passes them to the given closure. THe configuration system
/// checks at compile time that the borrowing rules are observed by the created
/// closures, and raises an error if the rules can't be statically enforced.
///
/// # Stability
///
/// This trait is covered by the application-side API stability guarantee with
/// a few exceptions, which are documented on a per-item basis.
///
/// [1]: Bind
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
    type Target<'call> = T;

    #[inline]
    unsafe fn materialize<'call>(self) -> Self::Target<'call> {
        unsafe { BindHunk::as_ref(self.0).0.get().read().assume_init() }
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

// ----------------------------------------------------------------------------
