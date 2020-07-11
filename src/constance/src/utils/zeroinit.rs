use core::{cell::UnsafeCell, mem, sync::atomic};

use super::RawCell;

/// Trait for zero-initializable types.
///
/// # Safety
///
/// Zero-initialization is not safe for all types. For example, references
/// (`&_`)
pub unsafe trait ZeroInit {}

unsafe impl<T> ZeroInit for atomic::AtomicPtr<T> {}

unsafe impl<T: ZeroInit> ZeroInit for UnsafeCell<T> {}
unsafe impl<T: ZeroInit> ZeroInit for RawCell<T> {}
unsafe impl<T> ZeroInit for mem::MaybeUninit<T> {}

unsafe impl<T: ZeroInit> ZeroInit for [T] {}

unsafe impl<T: ?Sized> ZeroInit for *const T {}
unsafe impl<T: ?Sized> ZeroInit for *mut T {}
unsafe impl<T: ?Sized> ZeroInit for Option<&'_ T> {}
unsafe impl<T: ?Sized> ZeroInit for Option<&'_ mut T> {}

macro_rules! impl_zero_init {
    (
        $(
            $( #[$meta:meta] )*
            $ty:ty,
        )*
    ) => {
        $(
            $( #[$meta] )*
            unsafe impl ZeroInit for $ty {
            }
        )*
    };
}

impl_zero_init! {
    bool,
    char,
    u8,
    u16,
    u32,
    u64,
    u128,
    i8,
    i16,
    i32,
    i64,
    i128,
    usize,
    isize,
    f32,
    f64,
    #[cfg(target_has_atomic_load_store = "8")]
    atomic::AtomicBool,
    #[cfg(target_has_atomic_load_store = "8")]
    atomic::AtomicU8,
    #[cfg(target_has_atomic_load_store = "16")]
    atomic::AtomicU16,
    #[cfg(target_has_atomic_load_store = "32")]
    atomic::AtomicU32,
    #[cfg(target_has_atomic_load_store = "64")]
    atomic::AtomicU64,
    #[cfg(target_has_atomic_load_store = "ptr")]
    atomic::AtomicUsize,
    #[cfg(target_has_atomic_load_store = "8")]
    atomic::AtomicI8,
    #[cfg(target_has_atomic_load_store = "16")]
    atomic::AtomicI16,
    #[cfg(target_has_atomic_load_store = "32")]
    atomic::AtomicI32,
    #[cfg(target_has_atomic_load_store = "64")]
    atomic::AtomicI64,
    #[cfg(target_has_atomic_load_store = "ptr")]
    atomic::AtomicIsize,
    (),
}

unsafe impl<T, const LEN: usize> ZeroInit for [T; LEN] where T: ZeroInit {}

macro_rules! fn_impl_zero_init {
    ( $h:ident, $($t:ident,)* ) => {
        unsafe impl<Ret, $h, $($t,)*> ZeroInit for Option<fn($h, $($t,)*) -> Ret> {
        }

        fn_impl_zero_init! { $($t,)* }
    };
    () => {};
}

fn_impl_zero_init! {
    A, B, C, D, E, F, G, H, I, J, K, L,
}
