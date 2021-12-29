//! Testing utilities

/// Keeps doc tests clean. Not for public consumption.
#[doc(hidden)]
pub(crate) macro doc_test(
    #[doc = r" ```rust"]
    $( #[doc = $doc:expr] )*
) {concat!(
" ```rust", ignore_if_port_std_does_not_support_target!(), "\n ",
"#   #![feature(const_fn_trait_bound)]
 #   #![feature(const_mut_refs)]
 #   #![feature(const_fn_fn_ptr_basics)]
 #   #![feature(const_trait_impl)]
 #   #![deny(unsafe_op_in_unsafe_fn)]
 #
 #   use std::process::exit;
 #   use r3::kernel::{Cfg, traits, prelude::*};
 #
 #   // `use_port!` generates `fn main()`, but the test harness cannot detect that
 #   #[cfg(any())]
 #   fn main() {}
 #
 #   r3_port_std::use_port!(unsafe struct SystemTraits);
 #   const COTTAGE: Objects =
 #       r3_kernel::build!(SystemTraits, configure_app => Objects);
 #   type System = r3_kernel::System<SystemTraits>;
",
$( $doc, "\n", )*
)}

// `r3_port_std`'s target support is limited
#[cfg(any(unix, windows))]
macro ignore_if_port_std_does_not_support_target() {
    ""
}

#[cfg(not(any(unix, windows)))]
macro ignore_if_port_std_does_not_support_target() {
    ",ignore"
}
