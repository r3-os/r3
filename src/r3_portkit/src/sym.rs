//! Simulates associated `static`s in traits.
// FIXME: Remove this module when real `static`s are supported in traits

/// Used as a parameter for a "function" whose content is actually a pointer
/// to a `static` item.
#[repr(C)]
#[doc(hidden)]
pub struct __UnsafeSymStaticMarker {
    _never_constructed: (),
}

#[doc(hidden)]
pub use core::{arch::asm, mem};

/// Define two `fn` items representing the specified `static` variable.
///
///  - `$fn_name` is a normal function just returning a `'static` reference to
///    that variable.
///
///  - `$sym_name` is not really a function but an immutable variable holding
///    the address of that variable. Inline assembly code can refer to this by a
///    `sym` operand, but when doing this, **it must append `_` (an underscore)
///    to the symbol name** (see the following example).
///
/// # Examples
///
/// ```
/// #![feature(naked_functions)]
/// #![feature(asm_const)]
/// #![feature(asm_sym)]
/// use r3_portkit::sym::sym_static;
/// use std::{arch::asm, cell::Cell};
///
/// struct InteriorMutable(Cell<usize>);
/// unsafe impl Sync for InteriorMutable {}
///
/// trait Tr {
///     sym_static!(#[sym(p_var)] fn var() -> &InteriorMutable);
/// }
///
/// static S: InteriorMutable = InteriorMutable(Cell::new(0));
///
/// impl Tr for u8 {
///     sym_static!(#[sym(p_var)] fn var() -> &InteriorMutable { &S });
/// }
///
/// // TODO: Replace with the following when const arguments become
/// //       inferrable:
/// //
/// //       let sr1: SymStatic<InteriorMutable, _> = u8::VAR;
/// let sr = u8::var();
/// sr.0.set(42);
/// assert_eq!(sr.0.get(), 42);
///
/// // Since it appears as a `fn` item, it can be passed to `asm!` by a
/// // `sym` input.
/// let got_value: usize;
/// #[cfg(target_arch = "x86_64")]
/// #[cfg(target_os = "linux")]
/// unsafe {
///     asm!("
///         mov {0}, qword ptr [rip + {1}_@GOTPCREL]
///         mov {0}, qword ptr [{0}]
///         mov {0}, qword ptr [{0}]",
///         out(reg) got_value, sym u8::p_var,
///     );
/// }
/// #[cfg(target_arch = "x86_64")]
/// #[cfg(not(target_os = "linux"))]
/// unsafe {
///     asm!("
///         mov {0}, qword ptr [rip + {1}_]
///         mov {0}, qword ptr [{0}]",
///         out(reg) got_value, sym u8::p_var,
///     )
/// };
/// #[cfg(target_arch = "aarch64")]
/// #[cfg(target_os = "macos")]
/// unsafe {
///     asm!("
///         adrp {0}, {1}_@PAGE
///         ldr {0}, [{0}, {1}_@PAGEOFF]
///         ldr {0}, [{0}]
///         ",
///         out(reg) got_value, sym u8::p_var
///     );
/// }
/// #[cfg(target_arch = "aarch64")]
/// #[cfg(not(target_os = "macos"))]
/// unsafe {
///     asm!("
///         adrp {0}, :got:{1}_
///         ldr {0}, [{0}, #:got_lo12:{1}_]
///         ldr {0}, [{0}]
///         ldr {0}, [{0}]
///         ",
///         out(reg) got_value, sym u8::p_var
///     )
/// };
/// assert_eq!(got_value, 42);
/// ```
pub macro sym_static {
    (
        // A trait item
        #[sym($sym_name:ident)]
        $(#[$meta:meta])*
        $vis:vis fn $fn_name:ident() -> &$ty:ty
    ) => {
        $(#[$meta])*
        #[allow(non_snake_case)]
        $vis fn $fn_name() -> &'static $ty;

        $(#[$meta])*
        #[allow(non_snake_case)]
        $vis unsafe extern "C" fn $sym_name(_: &'static $crate::sym::__UnsafeSymStaticMarker) -> !;
    },
    (
        // A concrete definition
        #[sym($sym_name:ident)]
        $(#[$meta:meta])*
        $vis:vis fn $fn_name:ident() -> &$ty:ty { &$static:path }
    ) =>{
        $(#[$meta])*
        #[inline(always)]
        $vis fn $fn_name() -> &'static $ty {
            &$static
        }

        $(#[$meta])*
        #[naked]
         #[cfg_attr(
            any(target_os = "macos", target_os = "ios"),
            link_section = "__DATA,__const"
        )]
        #[cfg_attr(
            not(any(target_os = "macos", target_os = "ios")),
            link_section = ".rodata"
        )]
        $vis unsafe extern "C" fn $sym_name(_: &'static $crate::sym::__UnsafeSymStaticMarker) -> ! {
            #[allow(unused_unsafe)]
            unsafe {
                $crate::sym::asm!(
                    "
                        .p2align {1}
                        .global {0}_
                        {0}_:
                        .dc.a {2}
                    ",
                    sym Self::$sym_name,
                    const $crate::sym::mem::align_of::<&'static $ty>().trailing_zeros(),
                    sym $static,
                    options(noreturn),
                );
            }
        }
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use std::{dbg, prelude::rust_2021::*};

    trait Tr {
        sym_static!(#[sym(p_var)] fn var() -> &u32);
    }

    static S0: u32 = 0;
    static S1: u32 = 1;
    static S2: u32 = 2;

    impl Tr for &'static u8 {
        sym_static!(
            #[sym(p_var)]
            fn var() -> &u32 {
                &S0
            }
        );
    }
    impl Tr for &'static u16 {
        sym_static!(
            #[sym(p_var)]
            fn var() -> &u32 {
                &S1
            }
        );
    }
    impl Tr for &'static u32 {
        sym_static!(
            #[sym(p_var)]
            fn var() -> &u32 {
                &S2
            }
        );
    }

    #[test]
    fn uniqueness() {
        let var1 = dbg!(<&'static u8>::var() as *const u32);
        let var2 = dbg!(<&'static u16>::var() as *const u32);
        let var3 = dbg!(<&'static u32>::var() as *const u32);
        assert_ne!(var1, var2);
        assert_ne!(var2, var3);
        assert_ne!(var1, var3);
    }
}
