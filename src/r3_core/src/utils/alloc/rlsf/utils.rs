use core::{mem::MaybeUninit, ptr::NonNull};

/// Polyfill for <https://github.com/rust-lang/rust/issues/71941>
#[inline]
pub const fn nonnull_slice_from_raw_parts<T>(ptr: NonNull<T>, len: usize) -> NonNull<[T]> {
    unsafe { NonNull::new_unchecked(core::ptr::slice_from_raw_parts_mut(ptr.as_ptr(), len)) }
}

/// Polyfill for  <https://github.com/rust-lang/rust/issues/71146>
#[inline]
pub const fn nonnull_slice_len<T>(ptr: NonNull<[T]>) -> usize {
    // Safety: We are just reading the slice length embedded in the fat
    //         pointer and not dereferencing the pointer. We also convert it
    //         to `*mut [MaybeUninit<u8>]` just in case because the slice
    //         might be uninitialized.
    unsafe { (*(ptr.as_ptr() as *const [MaybeUninit<T>])).len() }
}

// Polyfill for <https://github.com/rust-lang/rust/issues/74265>
#[inline]
pub const fn nonnull_slice_start<T>(ptr: NonNull<[T]>) -> NonNull<T> {
    unsafe { NonNull::new_unchecked(ptr.as_ptr() as *mut T) }
}

#[inline]
pub const fn nonnull_slice_end<T>(ptr: NonNull<[T]>) -> *mut T {
    (ptr.as_ptr() as *mut T).wrapping_add(nonnull_slice_len(ptr))
}

// FIXME: `usize: !const Ord`
pub const fn min_usize(x: usize, y: usize) -> usize {
    if x < y {
        x
    } else {
        y
    }
}

pub const fn option_nonnull_as_ptr<T>(x: Option<NonNull<T>>) -> *mut T {
    if let Some(x) = x {
        x.as_ptr()
    } else {
        core::ptr::null_mut()
    }
}
