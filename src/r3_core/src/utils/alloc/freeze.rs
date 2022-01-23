//! This module deals with interior mutability, which [undermines][1] the
//! soundness of runtime uses of `const`-allocated heap objects.
//!
//! [1]: https://github.com/rust-lang/rust/pull/91884#discussion_r774659436
use core::fmt;

// FIXME: Get rid of `Frozen` if one of the folllowing things happens:
//        (1) It's decided that interior mutability implies `!Copy`.
//        (2) A trait indicating the absence of interior mutability, such as
//        the one proposed by the now-closed [RFC 2944], is added to `core`.
//
// [RFC 2944]: https://github.com/rust-lang/rfcs/pull/2944
//
// <https://github.com/rust-lang/rust/issues/25053#issuecomment-493742957>:
//
// > Basically, currently we have no interior-mutable-`Copy` type, but that is
// > an accident. And we also have legit needs for `Copy` interior mutable
// > types, which is irreconcilable with using `Copy` to exclude interior
// > mutability.

/// Erases interior mutability by preventing reference forming.
#[repr(transparent)]
pub struct Frozen<T: ?Sized>(T);

impl<T: Copy> Frozen<T> {
    /// Get a copy of the contained `T`.
    #[inline]
    pub const fn get(&self) -> T {
        self.0
    }

    /// Copy the referenced `[T]` to the CTFE heap. The resulting reference can
    /// be safely consumed at runtime.
    ///
    /// # Example
    ///
    /// ```rust
    /// use r3_core::utils::Frozen;
    /// const SLICE: &[Frozen<u8>] = Frozen::leak_slice(&[1, 2, 3]);
    /// assert_eq!(SLICE[1].get(), 2);
    /// ```
    pub const fn leak_slice<'out>(x: &[T]) -> &'out [Frozen<T>]
    where
        T: 'out,
    {
        let size = core::mem::size_of::<T>()
            .checked_mul(x.len())
            .expect("size overflow");
        let align = core::mem::align_of::<T>();

        if size == 0 {
            return &[];
        }

        unsafe {
            // Allocate a CTFE heap memory block
            let ptr = core::intrinsics::const_allocate(size, align).cast::<T>();
            assert!(
                !ptr.guaranteed_eq(core::ptr::null_mut()),
                "heap allocation failed"
            );

            // Copy the `[T]` onto it
            core::ptr::copy_nonoverlapping(x.as_ptr(), ptr, x.len());

            // Reinterpret it as `[Frozen<T>]` (it's safe because of
            // `repr(transparent)`)
            let ptr = ptr.cast::<Frozen<T>>();

            // Turn `ptr` into a alice reference
            core::slice::from_raw_parts(ptr, x.len())
        }
    }
}

impl<T: Copy> Copy for Frozen<T> {}

impl<T: Copy> const Clone for Frozen<T> {
    #[inline]
    fn clone(&self) -> Self {
        // Don't use `T as Clone` because it could expose interior mutability.
        *self
    }

    #[inline]
    fn clone_from(&mut self, source: &Self) {
        *self = *source;
    }
}

impl<T: Copy + ~const fmt::Debug> const fmt::Debug for Frozen<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}
