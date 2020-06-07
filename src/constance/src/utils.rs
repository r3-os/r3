//! Utility

mod aligned_storage;
mod init;
mod rawcell;
mod zeroinit;
pub use self::{aligned_storage::*, init::*, rawcell::*, zeroinit::*};

/// A "type function" producing a type.
#[doc(hidden)]
pub trait TypeFn {
    type Output;
}
