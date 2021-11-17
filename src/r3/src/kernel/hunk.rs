//! Hunks
use core::{fmt, marker::PhantomData};

use super::{cfg, raw_cfg, Cfg};
use crate::utils::Init;

// ----------------------------------------------------------------------------

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
/// [`r3::hunk::Hunk`]: crate::hunk::Hunk
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
#[doc = include_str!("../common.md")]
pub struct Hunk<System: cfg::KernelStatic> {
    start: usize,
    _phantom: PhantomData<System>,
}

impl<System: cfg::KernelStatic> Init for Hunk<System> {
    const INIT: Self = Self::from_offset(0);
}

impl<System: cfg::KernelStatic> fmt::Debug for Hunk<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Hunk({:p})", self.as_ptr())
    }
}

impl<System: cfg::KernelStatic> Clone for Hunk<System> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<System: cfg::KernelStatic> Copy for Hunk<System> {}

impl<System: cfg::KernelStatic> Hunk<System> {
    /// Construct a `HunkDefiner` to define a hunk in [a
    /// configuration function](crate#static-configuration).
    pub const fn build() -> HunkDefiner<System> {
        HunkDefiner::new()
    }

    // I don't see any good reason to make this public, but the macro still
    // needs to access this
    #[doc(hidden)]
    #[inline]
    /// Construct a `Hunk` from `start` (offset in the kernel configuration's
    /// hunk pool).
    pub const fn from_offset(start: usize) -> Self {
        Self {
            start,
            _phantom: PhantomData,
        }
    }

    /// Get the offset of the hunk.
    #[inline]
    pub const fn offset(self) -> usize {
        self.start
    }
}

impl<System: cfg::KernelStatic> Hunk<System> {
    /// Get a raw pointer to the hunk's contents.
    #[inline]
    pub fn as_ptr(self) -> *mut u8 {
        System::hunk_pool_ptr().wrapping_add(self.start)
    }
}

// ----------------------------------------------------------------------------

/// The definer (static builder) for [`Hunk`].
#[must_use = "must call `finish()` to complete definition"]
pub struct HunkDefiner<System> {
    _phantom: PhantomData<System>,
    len: usize,
    align: usize,
}

impl<System: cfg::KernelStatic> HunkDefiner<System> {
    pub const fn new() -> Self {
        Self {
            _phantom: PhantomData,
            len: 0,
            align: 1,
        }
    }

    /// Specify the element count. Defaults to `0`.
    pub const fn len(self, len: usize) -> Self {
        Self { len, ..self }
    }

    /// Specify the minimum alignment. Defaults to `1`.
    pub const fn align(self, align: usize) -> Self {
        Self { align, ..self }
    }
}

impl<System: cfg::KernelStatic> HunkDefiner<System> {
    /// Complete the definition of a hunk, returning a reference to the hunk.
    pub const fn finish<C: raw_cfg::CfgBase>(self, cfg: &mut Cfg<C>) -> Hunk<System> {
        let Self { align, len, .. } = self;

        // Round up `hunk_pool_len`
        cfg.hunk_pool_len = (cfg.hunk_pool_len + align - 1) / align * align;

        let start = cfg.hunk_pool_len;

        cfg.hunk_pool_len += len;
        if align > cfg.hunk_pool_align {
            cfg.hunk_pool_align = align;
        }

        Hunk::from_offset(start)
    }
}
