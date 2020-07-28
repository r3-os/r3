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

impl<T: Init, const LEN: usize> Init for [T; LEN] {
    const INIT: Self = {
        let mut array = super::mem::uninit_array::<T, LEN>();

        // FIXME: Work-around for `for` being unsupported in `const fn`
        let mut i = 0;
        while i < LEN {
            array[i] = mem::MaybeUninit::new(T::INIT);
            i += 1;
        }

        // Safety: The memory layout of `[MaybeUninit<T>; LEN]` is
        // identical to `[T; LEN]`. We initialized all elements, so it's
        // safe to reinterpret that range as `[T; LEN]`.
        unsafe { super::mem::transmute(array) }
    };
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

impl<T: Init> Init for mem::ManuallyDrop<T> {
    const INIT: Self = mem::ManuallyDrop::new(T::INIT);
}

impl<T, const N: usize> Init for staticvec::StaticVec<T, N> {
    const INIT: Self = Self::new();
}

macro_rules! impl_init {
    (
        $(
            $( #[$meta:meta] )*
            $ty:ty => $value:expr,
        )*
    ) => {
        $(
            $( #[$meta] )*
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
    #[cfg(target_has_atomic_load_store = "8")]
    atomic::AtomicBool => atomic::AtomicBool::new(false),
    #[cfg(target_has_atomic_load_store = "8")]
    atomic::AtomicU8 => atomic::AtomicU8::new(0),
    #[cfg(target_has_atomic_load_store = "16")]
    atomic::AtomicU16 => atomic::AtomicU16::new(0),
    #[cfg(target_has_atomic_load_store = "32")]
    atomic::AtomicU32 => atomic::AtomicU32::new(0),
    #[cfg(target_has_atomic_load_store = "64")]
    atomic::AtomicU64 => atomic::AtomicU64::new(0),
    #[cfg(target_has_atomic_load_store = "ptr")]
    atomic::AtomicUsize => atomic::AtomicUsize::new(0),
    #[cfg(target_has_atomic_load_store = "8")]
    atomic::AtomicI8 => atomic::AtomicI8::new(0),
    #[cfg(target_has_atomic_load_store = "16")]
    atomic::AtomicI16 => atomic::AtomicI16::new(0),
    #[cfg(target_has_atomic_load_store = "32")]
    atomic::AtomicI32 => atomic::AtomicI32::new(0),
    #[cfg(target_has_atomic_load_store = "64")]
    atomic::AtomicI64 => atomic::AtomicI64::new(0),
    #[cfg(target_has_atomic_load_store = "ptr")]
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
