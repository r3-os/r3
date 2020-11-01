#[cfg(not(debug_assertions))]
use core::hint::unreachable_unchecked;

pub trait UnwrapUnchecked {
    type Output;

    /// Unwrap `self`, assuming `self` is unwrappable.
    ///
    /// This method may panic instead if `self` is not unwrappable and debug
    /// assertions are enabled.
    ///
    /// # Safety
    ///
    /// `self` must be `Ok(_)` or `Some(_)`.
    unsafe fn unwrap_unchecked(self) -> Self::Output;
}

impl<T> UnwrapUnchecked for Option<T> {
    type Output = T;

    #[inline]
    #[track_caller]
    #[cfg(debug_assertions)]
    unsafe fn unwrap_unchecked(self) -> Self::Output {
        self.unwrap()
    }

    #[inline]
    #[cfg(not(debug_assertions))]
    unsafe fn unwrap_unchecked(self) -> Self::Output {
        // Safety: `self` is `Some(_)`
        self.unwrap_or_else(|| unsafe { unreachable_unchecked() })
    }
}

impl<T, E: core::fmt::Debug> UnwrapUnchecked for Result<T, E> {
    type Output = T;

    #[inline]
    #[track_caller]
    #[cfg(debug_assertions)]
    unsafe fn unwrap_unchecked(self) -> Self::Output {
        self.unwrap()
    }

    #[inline]
    #[cfg(not(debug_assertions))]
    unsafe fn unwrap_unchecked(self) -> Self::Output {
        // Safety: `self` is `Some(_)`
        self.unwrap_or_else(|_| unsafe { unreachable_unchecked() })
    }
}
