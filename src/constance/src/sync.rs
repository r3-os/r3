//! Safe synchronization primitives.
use core::{cell::UnsafeCell, fmt, marker::PhantomData};

use crate::{
    kernel::{cfg::CfgBuilder, Hunk},
    prelude::*,
};

pub struct Mutex<System, T> {
    hunk: Hunk<System, UnsafeCell<T>>,
    _phantom: PhantomData<(System, T)>,
}

impl<System: Kernel, T: fmt::Debug + 'static> fmt::Debug for Mutex<System, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO: Display the contents if unlocked
        f.debug_struct("Mutex").field("hunk", &self.hunk).finish()
    }
}

impl<System: Kernel, T: 'static + Init> Mutex<System, T> {
    /// Construct a `Mutex`.
    ///
    /// This is a configuration function. Use `call!` inside `configure!` to
    /// use it.
    pub const fn new(b: &mut CfgBuilder<System>) -> Self {
        Self {
            hunk: Hunk::<_, UnsafeCell<T>>::build().finish(b),
            _phantom: PhantomData,
        }
    }
}
