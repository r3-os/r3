//! Safe synchronization primitives.
use core::{cell::UnsafeCell, fmt, marker::PhantomData};

use crate::{kernel::Hunk, prelude::*};

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
    configure! {
        /// Construct a `Mutex`.
        ///
        /// This is a configuration function. Use `call!` inside `configure!` to
        /// use it.
        pub const fn new(_: &mut CfgBuilder<System>) -> Self {
            Self {
                hunk: new_hunk!(UnsafeCell<T>),
                _phantom: PhantomData,
            }
        }
    }
}
