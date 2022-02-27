//! Provides [`Closure`], a light-weight closure type.
use core::{
    fmt,
    mem::{align_of, size_of},
};

use crate::utils::{mem::transmute, Init};

/// The environment parameter type for [`Closure`]. It's ABI-compatible with
/// `*mut ()` but might not be fully initialized.
///
/// It's something that would usually be just `intptr_t` or `void *` in C code.
/// It's designed to have the following properties:
///
///  - It's ABI-compatible with a C pointer, making it possible to pass the
///    components of a [`Closure`] to kernel implementations written in inline
///    assembly or other languages without needing to wrap it with another
///    trampoline.
///
///  - Unlike `dyn FnOnce()`, it doesn't waste memory for vtable, most entries
///    of which will never be used for static closures.
///
///  - Constructing it from a pointer doens't require a pointer-to-integer cast,
///    which is disallowed in a constant context.
///
/// Currently it must be filled with initialized bytes because of compiler
/// restrictions. This may change in the future.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct ClosureEnv(Option<&'static ()>);
// The contained type must be an initialized reference to avoid compile errors
// that occur with the current compiler. Ideally it should be
// `MaybeUninit<*mut ()>`, which, however, when a CTFE-heap allocation is
// stored, produces an enigmatic error "untyped pointers are not allowed in
// constant". [ref:const_untyped_pointer] [tag:closure_env_must_be_init]

impl const Default for ClosureEnv {
    #[inline]
    fn default() -> Self {
        Self::INIT
    }
}

impl Init for ClosureEnv {
    const INIT: Self = Self(None);
}

impl fmt::Debug for ClosureEnv {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ClosureEnv")
    }
}

// Want to parameterize, but there's no way to make the `fn` pointer
// high-ranked with generics [ref:generic_fn_ptr_wrapper]
//
/// A light-weight closure, which is comprised of a function pointer and an
/// environment parameter.
#[derive(Debug, Copy, Clone)]
pub struct Closure {
    /// The function pointer.
    func: unsafe extern "C" fn(ClosureEnv),
    env: ClosureEnv,
}

impl Init for Closure {
    const INIT: Closure = (|| {}).into_closure_const();
}

impl Default for Closure {
    #[inline]
    fn default() -> Self {
        Self::INIT
    }
}

impl Closure {
    /// Construct a `Self` from a function pointer and an associated pointer
    /// parameter.
    ///
    /// # Safety
    ///
    /// Safe code that has access to the constructed `Self` will be able to
    /// execute `func(env, _)`. A corollary is that, if `func` has additional
    /// safety requirements that are not covered by `Closure`, they are lost by
    /// this function, which means the resulting `Closure` mustn't be exposed to
    /// safe code.
    #[inline]
    pub const unsafe fn from_raw_parts(
        func: unsafe extern "C" fn(ClosureEnv),
        env: ClosureEnv,
    ) -> Self {
        Self { func, env }
    }

    /// Construct a `Self` from the given closure at compile time.
    ///
    /// The conversion may involve compile-time heap allocation
    /// ([`core::intrinsics::const_allocate`]). **It's illegal to call this
    /// function at runtime.**
    ///
    /// # Examples
    ///
    /// ```
    /// use r3_core::closure::Closure;
    ///
    /// // Zero-sized
    /// const C1: Closure = Closure::from_fn_const(|| {});
    ///
    /// // CTFE-heap-allocated
    /// const C2: Closure = {
    ///     let x = 42;
    ///     Closure::from_fn_const(move || assert_eq!(x, 42))
    /// };
    ///
    /// C1.call();
    /// C2.call();
    /// ```
    ///
    /// Don't call it at runtime:
    ///
    /// ```rust,should_panic
    /// use r3_core::closure::Closure;
    /// let x = [1, 2, 3];
    /// Closure::from_fn_const(move || { let _x = x; });
    /// ```
    pub const fn from_fn_const<T: FnOnce() + Copy + Send + 'static>(func: T) -> Self {
        let size = size_of::<T>();
        let align = align_of::<T>();
        unsafe {
            // FIXME: `ClosureEnv` can hold up to `size_of::<ClosureEnv>()`
            //        bytes in-line, but this can't be leveraged because its
            //        current representation requires that it be devoid of
            //        uninitialized bytes. [ref:closure_env_must_be_init]
            if size == 0 {
                Self::from_raw_parts(trampoline_zst::<T>, ClosureEnv(None))
            } else {
                let env = core::intrinsics::const_allocate(size, align);
                assert!(
                    !env.guaranteed_eq(core::ptr::null_mut()),
                    "heap allocation failed"
                );
                env.cast::<T>().write(func);
                Self::from_raw_parts(trampoline_indirect::<T>, transmute(env))
            }
        }
    }

    /// Call the closure.
    #[inline]
    pub fn call(self) {
        // Safety: `self.env` is provided as the first parameter
        unsafe { (self.func)(self.env) }
    }

    /// Get the function pointer.
    #[inline]
    pub const fn func(self) -> unsafe extern "C" fn(ClosureEnv) {
        self.func
    }

    /// Get the pojnter parameter.
    #[inline]
    pub const fn env(self) -> ClosureEnv {
        self.env
    }

    /// Decompose `self` into raw components.
    #[inline]
    pub const fn as_raw_parts(self) -> (unsafe extern "C" fn(ClosureEnv), ClosureEnv) {
        (self.func, self.env)
    }
}

#[inline]
unsafe extern "C" fn trampoline_zst<T: FnOnce()>(_: ClosureEnv) {
    let func: T = unsafe { transmute(()) };
    func()
}

#[inline]
unsafe extern "C" fn trampoline_indirect<T: FnOnce()>(env: ClosureEnv) {
    let p_func: *const T = unsafe { transmute(env) };
    // Since there's no trait indicating the lack of interior mutability,
    // we have to copy `T` onto stack. [ref:missing_interior_mutability_trait]
    let func: T = unsafe { p_func.read() };
    func()
}

/// A trait for converting a value into a [`Closure`] at compile time.
///
/// The conversion may involve compile-time heap allocation
/// ([`core::intrinsics::const_allocate`]). It's illegal to use this trait's
/// method at runtime.
///
/// # Examples
///
/// ```
/// #![feature(const_trait_impl)]
/// use r3_core::closure::{Closure, IntoClosureConst};
///
/// // `impl FnOnce()` → `Closure`
/// const _: Closure = (|| {}).into_closure_const();
///
/// // `(&'static P0, impl FnOnce(&'static P0))` → `Closure`
/// const _: Closure = (&42, |_: &i32| {}).into_closure_const();
///
/// // `(usize, impl FnOnce(usize))` → `Closure`
/// const _: Closure = (42usize, |_: usize| {}).into_closure_const();
/// ```
pub trait IntoClosureConst {
    /// Perform conversion to [`Closure`], potentially using a compile-time
    /// heap.
    fn into_closure_const(self) -> Closure;
}

impl const IntoClosureConst for Closure {
    fn into_closure_const(self) -> Closure {
        self
    }
}

/// Perform conversion using [`Closure::from_fn_const`].
impl<T: FnOnce() + Copy + Send + 'static> const IntoClosureConst for T {
    fn into_closure_const(self) -> Closure {
        Closure::from_fn_const(self)
    }
}

/// Packs `&P0` directly in [`ClosureEnv`][] if `T` is zero-sized.
///
/// Due to compiler restrictions, this optimization is currently impossible
/// to do in the generic constructor ([`Closure::from_fn_const`]).
// FIXME: See above
impl<T: FnOnce(&'static P0) + Copy + Send + 'static, P0: Sync + 'static> const IntoClosureConst
    for (&'static P0, T)
{
    fn into_closure_const(self) -> Closure {
        #[inline]
        unsafe extern "C" fn trampoline_ptr_spec<T: FnOnce(&'static P0), P0: 'static>(
            env: ClosureEnv,
        ) {
            let p0: &'static P0 = unsafe { transmute(env) };
            let func: T = unsafe { transmute(()) };
            func(p0)
        }

        if size_of::<T>() == 0 {
            unsafe { Closure::from_raw_parts(trampoline_ptr_spec::<T, P0>, transmute(self.0)) }
        } else {
            (move || (self.1)(self.0)).into_closure_const()
        }
    }
}

/// Packs `usize` directly in [`ClosureEnv`][] if `T` is zero-sized.
///
/// Due to compiler restrictions, this optimization is currently impossible
/// to do in the generic constructor ([`Closure::from_fn_const`]).
// FIXME: See above
impl<T: FnOnce(usize) + Copy + Send + 'static> const IntoClosureConst for (usize, T) {
    fn into_closure_const(self) -> Closure {
        #[inline]
        unsafe extern "C" fn trampoline_usize_spec<T: FnOnce(usize)>(env: ClosureEnv) {
            let p0: usize = unsafe { transmute(env) };
            let func: T = unsafe { transmute(()) };
            func(p0)
        }

        if size_of::<T>() == 0 {
            unsafe { Closure::from_raw_parts(trampoline_usize_spec::<T>, transmute(self.0)) }
        } else {
            (move || (self.1)(self.0)).into_closure_const()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn nested() {
        static STATE: AtomicUsize = AtomicUsize::new(0);

        const C1: Closure = {
            let value = 0x1234;
            (move || {
                STATE.fetch_add(value, Ordering::Relaxed);
            })
            .into_closure_const()
        };
        const C2: Closure = {
            let c = C1;
            (move || {
                c.call();
                c.call();
            })
            .into_closure_const()
        };
        const C3: Closure = {
            let c = C2;
            (move || {
                c.call();
                c.call();
            })
            .into_closure_const()
        };
        const C4: Closure = {
            let c = C3;
            (move || {
                c.call();
                c.call();
            })
            .into_closure_const()
        };

        STATE.store(0, Ordering::Relaxed);
        C4.call();
        assert_eq!(STATE.load(Ordering::Relaxed), 0x1234 * 8);
    }

    #[test]
    fn same_fn_different_env() {
        static STATE: AtomicUsize = AtomicUsize::new(0);

        const fn adder(x: usize) -> impl FnOnce() + Copy + Send {
            move || {
                STATE.fetch_add(x, Ordering::Relaxed);
            }
        }

        const ADD1: Closure = adder(1).into_closure_const();
        const ADD2: Closure = adder(2).into_closure_const();
        const ADD4: Closure = adder(4).into_closure_const();

        STATE.store(0, Ordering::Relaxed);
        ADD1.call();
        assert_eq!(STATE.load(Ordering::Relaxed), 1);
        ADD4.call();
        assert_eq!(STATE.load(Ordering::Relaxed), 1 + 4);
        ADD2.call();
        assert_eq!(STATE.load(Ordering::Relaxed), 1 + 4 + 2);
    }

    #[test]
    fn ptr_env_spec() {
        const C: Closure = (&42, |x: &i32| assert_eq!(*x, 42)).into_closure_const();
        C.call();
    }

    #[test]
    fn usize_env_spec() {
        const C: Closure = (42usize, |x: usize| assert_eq!(x, 42)).into_closure_const();
        C.call();
    }
}
