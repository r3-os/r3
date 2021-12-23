//! Utility
//!
//! **This module is exempt from the API stability guarantee** unless specified
//! otherwise. It's exposed only because it's needed by macros.
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

mod aligned_storage;
pub mod binary_heap;
pub(crate) mod convert;
mod ctz;
mod int;
pub(crate) mod intrusive_list;
pub mod mem;
pub(crate) mod pin;
mod prio_bitmap;
mod rawcell;
#[macro_use]
mod vec;
pub use self::{aligned_storage::*, int::*, prio_bitmap::*, rawcell::*, vec::*};
pub use r3_core::utils::{Init, ZeroInit};

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
