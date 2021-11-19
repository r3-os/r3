//! Hunks
use core::{fmt, marker::PhantomData};

use super::Kernel;
use crate::utils::Init;

/// Represents a single hunk in a system.
///
/// Hunks are nothing more than static variables defined in a kernel
/// configuration. They come in handy when you are designing a component that
/// can be instantiated by a kernel configuration and wanting each instance to
/// have its own separate state data.
///
/// This `Hunk` is untyped and only contains a starting address. See
/// [`r3::hunk::Hunk`] for a type-safe wrapper of `Hunk`.
///
/// [`r3::hunk::Hunk`]: crate::hunk::Hunk`
///
/// <div class="admonition-follows"></div>
///
/// > **Relation to Other Specifications:** None. The need for programmatically
/// > defining static regions has been traditionally fulfilled by code
/// > generation and preprocessor-based composition. The closest thing might be
/// > cell internal variables in the component system that comes with
/// > [the TOPPERS kernels].
///
/// [the TOPPERS kernels]: https://www.toppers.jp/index.html
#[doc = include_str!("./common.md")]
pub struct Hunk<System> {
    start: usize,
    _phantom: PhantomData<System>,
}

impl<System> Init for Hunk<System> {
    const INIT: Self = Self::from_offset(0);
}

impl<System: Kernel> fmt::Debug for Hunk<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Hunk({:p})", self.as_ptr())
    }
}

impl<System> Clone for Hunk<System> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<System> Copy for Hunk<System> {}

impl<System> Hunk<System> {
    // I don't see any good reason to make this public, but the macro still
    // needs to access this
    #[doc(hidden)]
    /// Construct a `Hunk` from `start` (offset in the kernel configuration's
    /// hunk pool).
    pub const fn from_offset(start: usize) -> Self {
        Self {
            start,
            _phantom: PhantomData,
        }
    }

    /// Get the offset of the hunk.
    pub const fn offset(self) -> usize {
        self.start
    }
}

impl<System: Kernel> Hunk<System> {
    /// Get a raw pointer to the hunk's contents.
    #[inline]
    pub fn as_ptr(self) -> *mut u8 {
        System::hunk_pool_ptr().wrapping_add(self.start)
    }
}
