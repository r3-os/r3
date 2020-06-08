//! Utility

mod aligned_storage;
mod init;
mod int;
mod rawcell;
mod zeroinit;
pub use self::{aligned_storage::*, init::*, rawcell::*, zeroinit::*, int::*};

/// A "type function" producing a type.
#[doc(hidden)]
pub trait TypeFn {
    type Output;
}
