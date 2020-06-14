use core::mem;

use crate::{
    kernel::{cfg::CfgBuilder, hunk},
    utils::{Init, ZeroInit},
};

/// Used by `new_hunk!` in configuraton functions
#[doc(hidden)]
pub const fn cfg_new_hunk<System, T: Init>(cfg: &mut CfgBuilder<System>) -> hunk::Hunk<System, T> {
    let align = mem::align_of::<T>();
    let size = mem::size_of::<T>();

    let inner = &mut cfg.inner;

    // Round up `hunk_pool_len`
    inner.hunk_pool_len = (inner.hunk_pool_len + align - 1) / align * align;

    let start = inner.hunk_pool_len;

    inner.hunks.push(hunk::HunkInitAttr {
        offset: start,
        init: |dest| unsafe {
            *(dest as *mut _) = T::INIT;
        },
    });

    inner.hunk_pool_len += size;
    if align > inner.hunk_pool_align {
        inner.hunk_pool_align = align;
    }

    unsafe { hunk::Hunk::from_range(start, size) }
}

/// Used by `new_hunk!` in configuraton functions
#[doc(hidden)]
pub const fn cfg_new_hunk_zero_array<System, T: ZeroInit>(
    cfg: &mut CfgBuilder<System>,
    len: usize,
    mut align: usize,
) -> hunk::Hunk<System, [T]> {
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

    // The hunk pool is zero-initialized by default
    let start = inner.hunk_pool_len;
    let hunk = unsafe { hunk::Hunk::from_range(start, byte_len) };
    inner.hunk_pool_len += byte_len;
    if align > inner.hunk_pool_align {
        inner.hunk_pool_align = align;
    }

    hunk
}
