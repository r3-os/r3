//! [`rlsf`][] modified for compile-time allocation.
//!
//! [`rlsf`]: https://crates.io/crates/rlsf
// FIXME: `rlsf` targets a pre-`unsafe_op_in_unsafe_fn` stable compiler, so
// it doesn't have `deny(unsafe_op_in_unsafe_fn)`. Eventually we need to remove
// this because this lint "may become [...] hard error in a future edition" [1].
//
// [1]: https://github.com/rust-lang/rust/blob/master/RELEASES.md#version-1520-2021-05-06
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]

macro_rules! const_panic {
    ($($tt:tt)*) => {
        panic!($($tt)*)
    };
}

macro_rules! const_try {
    ($x:expr) => {
        if let Some(x) = $x {
            x
        } else {
            return None;
        }
    };
}

macro debug_assert_eq($x:expr, $y:expr $(, $($tt:tt)*)?) {
    // FIXME: `*assert_eq!` is not usable in `const fn` yet
    debug_assert!($x == $y $(, $($tt)*)?);
}

macro debug_assert_eq_ptr($x:expr, $y:expr $(, $($tt:tt)*)?) {
    // FIXME: `*assert_eq!` is not usable in `const fn` yet
    debug_assert!(!$x.guaranteed_ne($y) $(, $($tt)*)?);
}

use crate::utils::Init;

mod flex;
pub mod int;
mod tlsf;
mod utils;
pub use self::{
    flex::*,
    tlsf::{Tlsf, ALIGN, GRANULARITY},
    utils::nonnull_slice_from_raw_parts,
};

#[cfg(test)]
mod tests;
