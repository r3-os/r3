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
        $($ty:ty,)*
    ) => {
        $(
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
    atomic::AtomicU8,
    atomic::AtomicU16,
    atomic::AtomicU32,
    atomic::AtomicU64,
    atomic::AtomicUsize,
    atomic::AtomicI8,
    atomic::AtomicI16,
    atomic::AtomicI32,
    atomic::AtomicI64,
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
