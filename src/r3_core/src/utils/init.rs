#![allow(clippy::declare_interior_mutable_const)]
use core::{
    cell::{Cell, RefCell, UnsafeCell},
    marker::PhantomData,
    mem,
    sync::atomic,
};

/// Trait for types having a constant default value. This is essentially a
/// constant version of `Default`.
///
/// This trait is subject to [the application-side API stability guarantee][1].
///
/// [1]: crate#stability
pub trait Init {
    /// The default value.
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

impl<T: ?Sized> Init for PhantomData<T> {
    const INIT: Self = PhantomData;
}

impl<T: Init, const LEN: usize> Init for [T; LEN] {
    const INIT: Self = {
        // `<[T; LEN]>::from_fn` is not `const fn` [ref:const_array_from_fn]
        let mut array = mem::MaybeUninit::uninit_array();

        // `[T]::iter` is unusable in `const fn` [ref:const_slice_iter]
        for i in 0..LEN {
            array[i] = mem::MaybeUninit::new(T::INIT);
        }

        // Safety: `array`'s elements are fully initialized
        unsafe { mem::MaybeUninit::array_assume_init(array) }
    };
}

impl<T> Init for atomic::AtomicPtr<T> {
    const INIT: Self = atomic::AtomicPtr::new(core::ptr::null_mut());
}

impl<T: Init> Init for UnsafeCell<T> {
    const INIT: Self = UnsafeCell::new(T::INIT);
}

impl<T: Init> Init for Cell<T> {
    const INIT: Self = Cell::new(T::INIT);
}

impl<T: Init> Init for RefCell<T> {
    const INIT: Self = RefCell::new(T::INIT);
}

impl<T: Init, I: Init> Init for tokenlock::TokenLock<T, I> {
    const INIT: Self = Self::new(I::INIT, T::INIT);
}

impl<T: Init, I: Init> Init for tokenlock::UnsyncTokenLock<T, I> {
    const INIT: Self = Self::new(I::INIT, T::INIT);
}

impl<Tag: ?Sized> Init for tokenlock::SingletonTokenId<Tag> {
    const INIT: Self = Self::new();
}

impl<T> Init for mem::MaybeUninit<T> {
    const INIT: Self = mem::MaybeUninit::uninit();
}

impl<T: Init> Init for mem::ManuallyDrop<T> {
    const INIT: Self = mem::ManuallyDrop::new(T::INIT);
}

impl<T, const N: usize> Init for arrayvec::ArrayVec<T, N> {
    const INIT: Self = Self::new_const();
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
