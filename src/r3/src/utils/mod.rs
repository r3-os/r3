//! Utility
use core::marker::PhantomData;

/// Conditional type
macro_rules! If {
    ( if ($cond:expr) { $t:ty } else { $f:ty } ) => {
        <crate::utils::Conditional<$t, $f, {$cond}> as crate::utils::TypeFn>::Output
    };
    (
        |$($cap:ident: $cap_ty:ty),* $(,)*|
        if ($cond:expr) { $t:ty } else { $f:ty }
    ) => {
        <crate::utils::Conditional<$t, $f, {
            // "Complex" expressions are not allowed in [generic constants][1],
            // but function calls are okay for some reasons
            //
            // [1]: https://github.com/rust-lang/rust/issues/76560
            #[allow(unused_variables, non_snake_case)]
            #[doc(hidden)]
            pub const fn __evaluate_condition($($cap: $cap_ty),*) -> bool {
                $cond
            }

            __evaluate_condition($($cap),*)
        }> as crate::utils::TypeFn>::Output
    };

    (
        $( |$($cap:ident: $cap_ty:ty),* $(,)*| )?
        if ($cond:expr) { $t:ty } else if $($rest:tt)* ) => {
        If! {
            $( |$($cap: $cap_ty),*| )?
            if ($cond) {
                $t
            } else {
                If!{ $( |$($cap: $cap_ty),*| )? if $($rest)* }
            }
        }
    };
}

#[macro_use]
mod binary_search;
#[macro_use]
mod sort;
pub(crate) use sort::*;
#[macro_use]
mod vec;
pub use vec::*;
#[macro_use]
pub mod for_times;

mod aligned_storage;
pub mod mem;
mod rawcell;
mod zeroinit;
pub use aligned_storage::*;
pub use rawcell::*;
pub use zeroinit::*;

#[doc(no_inline)]
pub use const_default::ConstDefault;

/// A phantom type that is invariant over `T`.
pub type PhantomInvariant<T> = core::marker::PhantomData<fn(T) -> T>;

/// A "type function" producing a type.
#[doc(hidden)]
pub trait TypeFn {
    type Output;
}

#[doc(hidden)]
pub struct Conditional<T, F, const B: bool>(PhantomData<(T, F)>);

impl<T, F> TypeFn for Conditional<T, F, false> {
    type Output = F;
}
impl<T, F> TypeFn for Conditional<T, F, true> {
    type Output = T;
}

/// Unwrap `#[doc = ...]`.
pub(crate) macro undoc(
    #[doc = $p0:expr]
    $( #[doc = $pN:expr] )*
) {
    concat!($p0, $( "\n", $pN, )*)
}
