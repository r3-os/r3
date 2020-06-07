//! Hunks
use core::{marker::PhantomData, mem::size_of, ops::Deref, ptr::slice_from_raw_parts};

use super::Kernel;
use crate::utils::Init;

/// Represents a single hunk in a system.
///
/// Hunks are nothing more than static variables defined in a kernel
/// configuration. They come in handy when you are designing a component that
/// can be instantiated by a kernel configuration and wanting each instance to
/// have its own separate state data.
pub struct Hunk<System, T: ?Sized> {
    start: usize,
    len: usize,
    _phantom: PhantomData<(System, T)>,
}

impl<System, T> Init for Hunk<System, [T]> {
    // Safety: This is safe because it points to nothing
    const INIT: Self = unsafe { Self::from_range(0, 0) };
}

impl<System, T: ?Sized> Clone for Hunk<System, T> {
    fn clone(&self) -> Self {
        Self {
            start: self.start,
            len: self.len,
            _phantom: PhantomData,
        }
    }
}

impl<System, T: ?Sized> Copy for Hunk<System, T> {}

impl<System, T: ?Sized> Hunk<System, T> {
    // I don't see any good reason to make this public, but the macro still
    // needs to access this
    #[doc(hidden)]
    /// Construct a `Hunk` from `start` (offset in the kernel configuration's
    /// hunk pool) and `len`.
    ///
    /// # Safety
    ///
    /// This method can invade the privacy of other components who want to be
    /// left alone.
    pub const unsafe fn from_range(start: usize, len: usize) -> Self {
        Self {
            start,
            len,
            _phantom: PhantomData,
        }
    }

    /// Reinterpret the hunk as another type.
    ///
    /// # Safety
    ///
    ///  - Similarly to [`core::mem::transmute`], this is **incredibly** unsafe.
    ///  - The byte offset and length must be valid for the destination type.
    ///
    pub const unsafe fn transmute<U: ?Sized>(self) -> Hunk<System, U> {
        Hunk {
            start: self.start,
            len: self.len,
            _phantom: PhantomData,
        }
    }
}

impl<System: Kernel, T: ?Sized> Hunk<System, T> {
    // FIXME: The following methods are not `const fn` on account of
    //        <https://github.com/rust-lang/const-eval/issues/11> being
    //        unresolved

    /// Get a raw pointer to the raw bytes of the hunk.
    pub fn as_bytes_ptr(this: Self) -> *const [u8] {
        slice_from_raw_parts(
            unsafe { System::HUNK_ATTR.hunk_pool_ptr().add(this.start) },
            this.len,
        )
    }

    /// Get a reference to the raw bytes of the hunk.
    ///
    /// # Safety
    ///
    /// The result might include uninitialized bytes and/or interior mutability,
    /// so it might be unsafe to access.
    pub unsafe fn as_bytes(this: Self) -> &'static [u8] {
        &*Self::as_bytes_ptr(this)
    }
}

impl<System: Kernel, T: 'static> Hunk<System, T> {
    /// Get a raw pointer to the hunk's contents.
    pub fn as_ptr(this: Self) -> *const T {
        Self::as_bytes_ptr(this) as *const T
    }
}

impl<System: Kernel, T: 'static> AsRef<T> for Hunk<System, T> {
    fn as_ref(&self) -> &T {
        unsafe { &*Self::as_ptr(*self) }
    }
}

impl<System: Kernel, T: 'static> Deref for Hunk<System, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<System: Kernel, T: 'static> Hunk<System, [T]> {
    /// Get a raw pointer to the hunk's contents.
    pub fn as_ptr(this: Self) -> *const [T] {
        slice_from_raw_parts(
            Self::as_bytes_ptr(this) as *const T,
            this.len / size_of::<T>(),
        )
    }
}

impl<System: Kernel, T: 'static> AsRef<[T]> for Hunk<System, [T]> {
    fn as_ref(&self) -> &[T] {
        unsafe { &*Self::as_ptr(*self) }
    }
}

impl<System: Kernel, T: 'static> Deref for Hunk<System, [T]> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

/// The static properties of hunks.
///
/// This type isn't technically public but needs to be `pub` so that it can be
/// referred to by a macro.
#[doc(hidden)]
pub struct HunkAttr {
    // FIXME: Waiting for <https://github.com/rust-lang/const-eval/issues/11>
    //        to be resolved
    pub hunk_pool: fn() -> *const u8,
    pub inits: &'static [HunkInitAttr],
}

impl HunkAttr {
    #[inline(always)]
    fn hunk_pool_ptr(&self) -> *const u8 {
        (self.hunk_pool)()
    }

    /// Initialize hunks.
    ///
    /// # Safety
    ///
    /// - Assumes `HunkInitAttr` points to memory regions within `hunk_pool`.
    /// - Assumes `hunk_pool` is currently not in use by user code.
    ///
    pub unsafe fn init_hunks(&self) {
        for init in self.inits.iter() {
            (init.init)(self.hunk_pool_ptr().add(init.offset) as *mut u8);
        }
    }
}

/// Initialize hunks.
///
/// This is meant to be called only once when a port is initializing the
/// execution environment.
///
/// # Safety
///
/// - Assumes `HunkInitAttr` points to memory regions within `hunk_pool`.
/// - Assumes `hunk_pool` is currently not in use by user code.
///
pub unsafe fn init_hunks<System: Kernel>() {
    System::HUNK_ATTR.init_hunks();
}

/// Hunk initializer.
///
/// This type isn't technically public but needs to be `pub` so that it can be
/// referred to by a macro.
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct HunkInitAttr {
    pub(super) offset: usize,
    pub(super) init: unsafe fn(*mut u8),
}

impl Init for HunkInitAttr {
    const INIT: Self = Self {
        offset: 0,
        init: |_| {},
    };
}
