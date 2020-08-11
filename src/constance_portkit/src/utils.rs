//! Utility
//!
//! **This module is exempt from the API stability guarantee** unless specified
//! otherwise. It's exposed only because it's needed by conditional types.
use core::marker::PhantomData;

#[doc(no_inline)]
pub use constance::utils::Init;

/// Conditional type
macro_rules! If {
    ( if ($cond:expr) { $t:ty } else { $f:ty } ) => {
        <crate::utils::Conditional<$t, $f, $cond> as crate::utils::TypeFn>::Output
    };
    ( if ($cond:expr) { $t:ty } else if $($rest:tt)* ) => {
        If! { if ($cond) { $t } else { If!{ if $($rest)* } } }
    };
}

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
