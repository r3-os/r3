//! Utility
//!
//! **This module is exempt from the API stability guarantee** unless specified
//! otherwise. It's exposed only because it's needed by macros.
use core::marker::PhantomData;

/// Conditional type
macro_rules! If {
    ( if ($cond:expr) { $t:ty } else { $f:ty } ) => {
        <crate::utils::Conditional<$t, $f, $cond> as crate::utils::TypeFn>::Output
    };
    ( if ($cond:expr) { $t:ty } else if $($rest:tt)* ) => {
        If! { if ($cond) { $t } else { If!{ if $($rest)* } } }
    };
}

#[macro_use]
mod binary_search;
#[macro_use]
mod sort;
mod aligned_storage;
pub mod binary_heap;
pub(crate) mod convert;
mod init;
mod int;
pub mod intrusive_list;
mod prio_bitmap;
mod rawcell;
#[macro_use]
mod vec;
#[macro_use]
pub mod for_times;
mod zeroinit;
pub use self::{
    aligned_storage::*, init::*, int::*, prio_bitmap::*, rawcell::*, vec::*, zeroinit::*,
};

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
