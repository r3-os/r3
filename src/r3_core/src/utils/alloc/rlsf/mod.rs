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

use crate::utils::Init;

mod flex;
pub mod int;
mod tlsf;
mod utils;
pub use self::{
    flex::*,
    tlsf::{Tlsf, GRANULARITY},
};

#[cfg(test)]
mod tests;
