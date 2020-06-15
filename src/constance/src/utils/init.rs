use core::{cell::UnsafeCell, mem, sync::atomic};

use super::RawCell;

/// Trait for types having a constant default value. This is essentially a
/// constant version of `Default`.
///
/// This trait is subject to the API stability guarantee.
pub trait Init {
    /// The default value.
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self;
}

impl<T: 'static> Init for &'_ [T] {
    const INIT: Self = &[];
}

impl Init for &'_ str {
    const INIT: Self = "";
}

impl<T> Init for Option<T> {
    const INIT: Self = None;
}

impl<T> Init for atomic::AtomicPtr<T> {
    const INIT: Self = atomic::AtomicPtr::new(core::ptr::null_mut());
}

impl<T: Init> Init for UnsafeCell<T> {
    const INIT: Self = UnsafeCell::new(T::INIT);
}

impl<T: Init> Init for RawCell<T> {
    const INIT: Self = RawCell::new(T::INIT);
}

impl<T: Init, I: Init> Init for tokenlock::TokenLock<T, I> {
    const INIT: Self = Self::new(I::INIT, T::INIT);
}

impl<T> Init for mem::MaybeUninit<T> {
    const INIT: Self = mem::MaybeUninit::uninit();
}

macro_rules! impl_init {
    (
        $($ty:ty => $value:expr,)*
    ) => {
        $(
            impl Init for $ty {
                const INIT: Self = $value;
            }
        )*
    };
}

impl_init! {
    bool => false,
    char => '\0',
    u8 => 0,
    u16 => 0,
    u32 => 0,
    u64 => 0,
    u128 => 0,
    i8 => 0,
    i16 => 0,
    i32 => 0,
    i64 => 0,
    i128 => 0,
    usize => 0,
    isize => 0,
    f32 => 0.0,
    f64 => 0.0,
    atomic::AtomicU8 => atomic::AtomicU8::new(0),
    atomic::AtomicU16=> atomic::AtomicU16::new(0),
    atomic::AtomicU32 => atomic::AtomicU32::new(0),
    atomic::AtomicU64 => atomic::AtomicU64::new(0),
    atomic::AtomicUsize => atomic::AtomicUsize::new(0),
    atomic::AtomicI8 => atomic::AtomicI8::new(0),
    atomic::AtomicI16 => atomic::AtomicI16::new(0),
    atomic::AtomicI32 => atomic::AtomicI32::new(0),
    atomic::AtomicI64 => atomic::AtomicI64::new(0),
    atomic::AtomicIsize => atomic::AtomicIsize::new(0),
    () => (),
}

macro_rules! tuple_impl_init {
    ( $h:ident, $($t:ident,)* ) => {
        impl<$h: Init, $($t: Init,)*> Init for ($h, $($t,)*) {
            const INIT: Self = (
                $h::INIT,
                $($t::INIT,)*
            );
        }

        tuple_impl_init! { $($t,)* }
    };
    () => {};
}

tuple_impl_init! {
    A, B, C, D, E, F, G, H, I, J, K, L,
}

macro_rules! array_impl_init {
    {$n:expr, $t:ident $($ts:ident)*} => {
        impl<T> Init for [T; $n] where T: Init {
            const INIT: Self = [$t::INIT, $($ts::INIT),*];
        }
        array_impl_init!{($n - 1), $($ts)*}
    };
    {$n:expr,} => {
        impl<T> Init for [T; $n] {
            const INIT: Self = [];
        }
    };
}

array_impl_init! {32, T T T T T T T T T T T T T T T T T T T T T T T T T T T T T T T T}
