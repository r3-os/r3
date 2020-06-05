//! Safe synchronization primitives.
use core::marker::PhantomData;

use crate::prelude::*;

pub struct Mutex<System, T> {
    _phantom: PhantomData<(System, T)>,
}

impl<System: Kernel, T> Mutex<System, T> {
    configure! {
        /// Construct a `Mutex`.
        ///
        /// This is a configure function. Use `call!` inside `configure!` to
        /// use it.
        pub fn new(_: CfgBuilder<System>) -> Self {
            Self {
                _phantom: PhantomData,
            }
        }
    }
}
