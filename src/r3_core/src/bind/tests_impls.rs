//! The test code for the trait implementations and the lack thereof.
//!
//! We don't use `static_assertions` here because it's non-trivial to create a
//! concrete system type.
//!
//! For negative tests, we use doc tests, which are a simple way to assert the
//! lack of certain trait implementations. There's a caveat: the doc tests must
//! must be visible with `cfg(not(test))`.
#[cfg(test)]
#[allow(dead_code)]
use super::*;

macro_rules! assert_compile_fail {
    ($($tt:tt)*) => {
        const _: () = {
            /// ```rust,compile_fail
            #[doc = stringify!($($tt)*)]
            /// ```
            #[allow(dead_code)]
            fn assert_compile_fail() {}
        };
    };
}

macro_rules! assert_fn_bind_impls {
    () => {};
    (
        (<$ty:ty>: !$($bounds:tt)*),
        $($rest:tt)*
    ) => {
        assert_compile_fail! {
            use r3_core::bind::*;
            use r3_core::kernel::{raw, cfg};

            fn assert_fn_bind_impl<System, T, T0, T1, T2>(
                x: $ty
            ) -> impl $($bounds)*
            where
                System: raw::KernelBase + cfg::KernelStatic,
                T: 'static + Sync,
                T0: 'static + Sync,
                T1: 'static + Sync,
                T2: 'static + Sync,
            {
                x
            }
        }

        // Catch invalid syntax; we don't want the compile-fail test to pass
        // because of that
        #[cfg(test)]
        const _: () = {
            fn assert_valid_syntax<System, T, T0, T1, T2, Output>(
                _: $ty
            )
            where
                System: raw::KernelBase + cfg::KernelStatic,
                T: 'static + Sync,
                T0: 'static + Sync,
                T1: 'static + Sync,
                T2: 'static + Sync,
                Output: $($bounds)*,
            {
                todo!()
            }
        };

        assert_fn_bind_impls! { $($rest)* }
    };
    (
        (<$ty:ty>: $($bounds:tt)*),
        $($rest:tt)*
    ) => {
        #[cfg(test)]
        const _: () = {
            fn assert_fn_bind_impl<System, T, T0, T1, T2>(
                x: $ty
            ) -> impl $($bounds)*
            where
                System: raw::KernelBase + cfg::KernelStatic,
                T: 'static + Sync,
                T0: 'static + Sync,
                T1: 'static + Sync,
                T2: 'static + Sync,
            {
                x
            }
        };

        assert_fn_bind_impls! { $($rest)* }
    };
}

assert_fn_bind_impls! {
    (<fn()>: FnBindAsClosure<()>),
    (<fn()>: !FnBindAsClosure<(BindRef<System, T>,)>),

    (<fn(&T)>: !FnBindAsClosure<()>),
    (<fn(&T)>: FnBindAsClosure<(BindRef<System, T>,)>),
    (<fn(&T)>: FnBindAsClosure<(BindBorrow<'static, System, T>,)>),
    (<fn(&T)>: !FnBindAsClosure<(BindRef<System, T0>,)>),

    (<fn(&T0, &T1)>: !FnBindAsClosure<()>),
    (<fn(&T0, &T1)>: !FnBindAsClosure<(BindRef<System, T>,)>),
    (<fn(&T0, &T1)>: !FnBindAsClosure<(BindRef<System, T0>,)>),
    (<fn(&T0, &T1)>: !FnBindAsClosure<(BindRef<System, T1>,)>),
    (<fn(&T0, &T1)>: FnBindAsClosure<(BindRef<System, T0>, BindRef<System, T1>)>),
    (<fn(&T0, &T1)>: FnBindAsClosure<(
        BindBorrow<'static, System, T0>,
        BindBorrow<'static, System, T1>)>),
    (<fn(&T0, &T1)>: !FnBindAsClosure<(BindRef<System, T1>, BindRef<System, T0>)>),

    (<for<'a> fn(&'a T0, &'a T1)>:
        FnBindAsClosure<(BindRef<System, T0>, BindRef<System, T1>)>),
    (<for<'a> fn(&'a T0, &'a T1)>:
        FnBindAsClosure<(
            BindBorrow<'static, System, T0>,
            BindBorrow<'static, System, T1>)>),

    // The function requires a `'static` reference
    (<fn(&'static T)>: FnBindAsClosure<(BindRef<System, T>,)>),
    (<fn(&'static T)>: !FnBindAsClosure<(BindBorrow<'static, System, T>,)>),
}
