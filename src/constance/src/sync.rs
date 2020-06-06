//! Safe synchronization primitives.
use core::{cell::UnsafeCell, marker::PhantomData};

use crate::{kernel::Hunk, prelude::*};

pub struct Mutex<System, T> {
    hunk: Hunk<System, UnsafeCell<T>>,
    _phantom: PhantomData<(System, T)>,
}

impl<System: Kernel, T: 'static + Init> Mutex<System, T> {
    configure! {
        /// Construct a `Mutex`.
        ///
        /// This is a configuration function. Use `call!` inside `configure!` to
        /// use it.
        pub fn new(_: CfgBuilder<System>) -> Self {
            Self {
                hunk: new_hunk!(UnsafeCell<T>),
                _phantom: PhantomData,
            }
        }
    }
}
