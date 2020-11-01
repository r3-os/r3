use core::marker::PhantomData;

use crate::kernel::{cfg::CfgBuilder, hunk, Port};

impl<System: Port> hunk::Hunk<System> {
    /// Construct a `CfgTaskBuilder` to define a hunk in [a configuration
    /// function](crate#static-configuration).
    pub const fn build() -> CfgHunkBuilder<System> {
        CfgHunkBuilder {
            _phantom: PhantomData,
            len: 1,
            align: 1,
        }
    }
}

/// Configuration builder type for [`Hunk`].
///
/// [`Hunk`]: crate::kernel::Hunk
#[must_use = "must call `finish()` to complete registration"]
pub struct CfgHunkBuilder<System> {
    _phantom: PhantomData<System>,
    len: usize,
    align: usize,
}

impl<System: Port> CfgHunkBuilder<System> {
    /// Specify the element count. Defaults to `1`. Must be `1` for a non-array
    /// hunk.
    pub const fn len(self, len: usize) -> Self {
        Self { len, ..self }
    }

    /// Specify the minimum alignment. Defaults to `1`.
    pub const fn align(self, align: usize) -> Self {
        Self { align, ..self }
    }
}

impl<System: Port> CfgHunkBuilder<System> {
    /// Complete the definition of a hunk, returning a reference to the hunk.
    pub const fn finish(self, cfg: &mut CfgBuilder<System>) -> hunk::Hunk<System> {
        let Self { align, len, .. } = self;
        let inner = &mut cfg.inner;

        // Round up `hunk_pool_len`
        inner.hunk_pool_len = (inner.hunk_pool_len + align - 1) / align * align;

        let start = inner.hunk_pool_len;

        inner.hunk_pool_len += len;
        if align > inner.hunk_pool_align {
            inner.hunk_pool_align = align;
        }

        hunk::Hunk::from_offset(start)
    }
}
