//! Utility
//!
//! **This module is exempt from [the API stability guarantee][1]** unless
//! specified otherwise. It's exposed mostly because it's needed by macros.
//!
//! # Re-exports
//!
//! - [`Zeroable`], [`ZeroableInOption`]: Re-exported from [`bytemuck`]` ^1`.
//!   `Zeroable` is a marker trait used in [`HunkDefiner::zeroed`][2] and
//!   suchlike. It can be derived for struct types.
//!   These re-exports are subject to the application-side API stability
//!   guarantee.
//!
//! [1]: crate#stability
//! [2]: crate::hunk::HunkDefiner::zeroed
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
mod freeze;
pub use freeze::*;
mod alloc;
pub use alloc::*;
#[macro_use]
pub mod for_times;

mod aligned_storage;
pub(crate) mod binary_heap;
mod init;
pub mod mem;
mod rawcell;
pub(crate) mod refcell;
pub use aligned_storage::*;
pub use init::*;
pub use rawcell::*;

pub use bytemuck::{Zeroable, ZeroableInOption};

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
