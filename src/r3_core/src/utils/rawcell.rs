use core::cell::UnsafeCell;

use crate::utils::{Init, Zeroable};

/// Like `UnsafeCell`, but implements `Sync`.
#[derive(Debug)]
#[repr(transparent)]
pub struct RawCell<T: ?Sized>(UnsafeCell<T>);

unsafe impl<T: Sync + ?Sized> Sync for RawCell<T> {}

impl<T> RawCell<T> {
    pub const fn new(x: T) -> Self {
        Self(UnsafeCell::new(x))
    }

    pub fn into_inner(self) -> T {
        self.0.into_inner()
    }
}

impl<T: ?Sized> RawCell<T> {
    pub const fn get(&self) -> *mut T {
        self.0.get()
    }
}

impl<T: Init> Init for RawCell<T> {
    const INIT: Self = RawCell::new(T::INIT);
}
// FIXME: Derive this when <https://github.com/Lokathor/bytemuck/pull/148> is
//        merged
unsafe impl<T: Zeroable> Zeroable for RawCell<T> {}
