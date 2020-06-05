#![feature(external_doc)] // `#[doc(include = ...)]`
#![feature(const_fn)]
#![feature(const_if_match)]
#![feature(const_panic)]
#![feature(const_loop)]
#![feature(const_generics)]
#![no_std]
#![doc(include = "./lib.md")]

pub mod kernel;
pub mod utils;

/// The prelude module.
pub mod prelude {
    #[doc(no_inline)]
    pub use crate::{kernel::Kernel, utils::Init};
}
