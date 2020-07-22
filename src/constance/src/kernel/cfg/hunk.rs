use core::{marker::PhantomData, mem};

use crate::{
    kernel::{cfg::CfgBuilder, hunk, Port},
    utils::{Init, ZeroInit},
};

impl<System: Port, T: ?Sized> hunk::Hunk<System, T> {
    /// Construct a `CfgTaskBuilder` to define a hunk in [a configuration
    /// function](crate#static-configuration).
    pub const fn build() -> CfgHunkBuilder<System, T, DefaultInitTag> {
        CfgHunkBuilder {
            _phantom: PhantomData,
            len: 1,
            align: 1,
        }
    }
}

/// As a generic parameter of [`CfgHunkBuilder`], indicates that the [hunk]
/// should be initialized with [`Init`].
///
/// [`Init`]: crate::utils::Init
/// [the hunk]: crate::kernel::Hunk
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct DefaultInitTag;

/// As a generic parameter of [`CfgHunkBuilder`], indicates that the [hunk]
/// should be zero-initialized.
///
/// [the hunk]: crate::kernel::Hunk
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct ZeroInitTag;

/// Implemented on [`DefaultInitTag`] and [`ZeroInitTag`] when `T` can be
/// initialized in this way.
pub trait HunkIniter<T> {
    /// A flag indicating whether [`Self::init`] should be called for
    /// initialization.
    const NEEDS_INIT: bool;

    /// Initialize the specified memory region.
    fn init(dest: &mut mem::MaybeUninit<T>);
}

impl<T: Init> HunkIniter<T> for DefaultInitTag {
    const NEEDS_INIT: bool = true;
    fn init(dest: &mut mem::MaybeUninit<T>) {
        *dest = mem::MaybeUninit::new(T::INIT);
    }
}

impl<T> HunkIniter<T> for ZeroInitTag {
    const NEEDS_INIT: bool = false;
    fn init(_: &mut mem::MaybeUninit<T>) {
        // Do nothing - a hunk pool is zero-initialized by default
    }
}

/// Configuration builder type for [`Hunk`].
///
/// `InitTag` is either [`DefaultInitTag`] or [`ZeroInitTag`].
///
/// [`Hunk`]: crate::kernel::Hunk
#[must_use = "must call `finish()` to complete registration"]
pub struct CfgHunkBuilder<System, T: ?Sized, InitTag> {
    _phantom: PhantomData<(System, InitTag, T)>,
    len: usize,
    align: usize,
}

impl<System: Port, T: ?Sized, InitTag> CfgHunkBuilder<System, T, InitTag> {
    /// Specify the element count. Defaults to `1`. Must be `1` for a non-array
    /// hunk.
    pub const fn len(self, len: usize) -> Self {
        Self { len, ..self }
    }

    /// Specify the minimum alignment. Defaults to `1`.
    pub const fn align(self, align: usize) -> Self {
        Self { align, ..self }
    }

    /// Zero-initialize the hunk.
    pub const fn zeroed(self) -> CfgHunkBuilder<System, T, ZeroInitTag>
    where
        T: ZeroInit,
    {
        // Safety: `T: ZeroInit`, so it's zero-initializable
        unsafe { self.zeroed_unchecked() }
    }

    /// Zero-initialize the hunk even if it might be unsafe.
    ///
    /// # Safety
    ///
    /// If zero initialization is not a valid bit pattern for `T`, accessing the
    /// hunk's contents may result in an undefined behavior.
    pub const unsafe fn zeroed_unchecked(self) -> CfgHunkBuilder<System, T, ZeroInitTag> {
        CfgHunkBuilder {
            _phantom: PhantomData,
            len: self.len,
            align: self.align,
        }
    }
}

impl<System: Port, T, InitTag: HunkIniter<T>> CfgHunkBuilder<System, T, InitTag> {
    /// Complete the definition of a hunk, returning a reference to the hunk.
    pub const fn finish(self, cfg: &mut CfgBuilder<System>) -> hunk::Hunk<System, T> {
        let align = mem::align_of::<T>();
        let size = mem::size_of::<T>();

        let inner = &mut cfg.inner;

        if self.len != 1 {
            panic!("Non-array hunk must have `len` of `1`");
        }

        // Round up `hunk_pool_len`
        inner.hunk_pool_len = (inner.hunk_pool_len + align - 1) / align * align;

        let start = inner.hunk_pool_len;

        // Insert an initializer
        if InitTag::NEEDS_INIT {
            inner.hunks.push(hunk::HunkInitAttr {
                offset: start,
                init: |dest: *mut u8| {
                    // Safety: The destination is large enough to contain `T`
                    InitTag::init(unsafe { &mut *(dest as *mut mem::MaybeUninit<T>) });
                },
            });
        }

        inner.hunk_pool_len += size;
        if align > inner.hunk_pool_align {
            inner.hunk_pool_align = align;
        }

        unsafe { hunk::Hunk::from_range(start, size) }
    }
}

impl<System: Port, T, InitTag: HunkIniter<T>> CfgHunkBuilder<System, [T], InitTag> {
    /// Complete the definition of a hunk, returning a reference to the hunk.
    pub const fn finish(self, cfg: &mut CfgBuilder<System>) -> hunk::Hunk<System, [T]> {
        let Self { mut align, len, .. } = self;
        let inner = &mut cfg.inner;

        if !align.is_power_of_two() {
            panic!("`align` is not power of two");
        }

        if mem::align_of::<T>() > align {
            align = mem::align_of::<T>();
        }

        let byte_len = mem::size_of::<T>() * len;

        // Round up `hunk_pool_len`
        inner.hunk_pool_len = (inner.hunk_pool_len + align - 1) / align * align;

        let start = inner.hunk_pool_len;

        // Insert an initializer
        if InitTag::NEEDS_INIT {
            // TODO: There is no way to pass a length into the initializer
            todo!();
        }

        inner.hunk_pool_len += byte_len;
        if align > inner.hunk_pool_align {
            inner.hunk_pool_align = align;
        }

        unsafe { hunk::Hunk::from_range(start, byte_len) }
    }
}
