//! Simulates associated `static`s in traits.
// FIXME: Remove this module when real `static`s are supported in traits
use core::{marker::PhantomData, mem::align_of};
use r3::utils::ZeroInit;

/// Used as a parameter for the [`SymStatic`] function pointer type.
///
/// An external crate must not name this type. Defining a function with
/// a signature matching [`SymStatic`] is unsafe.
#[repr(C)]
#[doc(hidden)]
pub struct __UnsafeSymStaticMarker<T: 'static, const ALIGN: usize>(PhantomData<&'static T>);

#[doc(hidden)]
pub use core::{arch::asm, mem};

/// Represents a value of `&'static T`. The functions of this type are defined
/// by [`sym_static!`].
///
/// **The `ALIGN` parameter and its existence or non-existence are exempt from
/// the API stability guarantee.** For now, use [`sym_static()`] to coerce a
/// value into this type without explicitly specifying its parameters.
pub type SymStatic<T, const ALIGN: usize> =
    unsafe extern "C" fn(&'static __UnsafeSymStaticMarker<T, ALIGN>) -> !;

/// Coerce a value into [`SymStatic`].
pub const fn sym_static<T: 'static, const ALIGN: usize>(
    x: SymStatic<T, ALIGN>,
) -> SymStatic<T, ALIGN> {
    x
}

pub unsafe trait SymStaticExt: Sized {
    type Output;

    /// Get a raw pointer of the content.
    fn as_ptr(self) -> *const Self::Output;

    /// Get a reference of the content.
    fn as_ref(self) -> &'static Self::Output
    where
        Self::Output: Sync + ZeroInit,
    {
        unsafe { &*self.as_ptr() }
    }
}

unsafe impl<T, const ALIGN: usize> SymStaticExt for SymStatic<T, ALIGN> {
    type Output = T;

    #[inline(always)]
    #[cfg(all(target_arch = "arm", target_feature = "thumb-mode"))]
    fn as_ptr(self) -> *const T {
        // Remove the Thumb flag by subtracting by one.
        round_up(self as usize - 1, align_of::<T>(), ALIGN) as *const T
    }

    #[inline(always)]
    #[cfg(not(all(target_arch = "arm", target_feature = "thumb-mode")))]
    fn as_ptr(self) -> *const T {
        round_up(self as usize, align_of::<T>(), ALIGN) as *const T
    }
}

/// Skip the `.p2align log2(align)` directive (used by [`sym_static!`]) to
/// locate the actual address where the variable is stored.
///
/// ```text
///     .p2align log2(fn_align)     // implicit
/// ptr:
///     .p2align log2(align)        // explicitly produced in `sym_static!(...)`
/// returned_ptr:
///     .zero size_of_T
/// ```
///
/// If the variable type's alignment requirement (`align`) is at least as
/// liberal as `fn_align`, the `.p2align` directive has no effect and therefore
/// this function can be no-op.
#[inline(always)]
fn round_up(ptr: usize, align: usize, fn_align: usize) -> usize {
    if align > fn_align {
        (ptr + align - 1) / align * align
    } else {
        ptr
    }
}

/// Functions are naturally aligned by this value.
#[doc(hidden)]
#[allow(clippy::if_same_then_else)] // misfires because clippy expands `cfg!` first?
pub const DEFAULT_FN_ALIGN: usize = if cfg!(target_arch = "aarch64") {
    4
} else if cfg!(target_arch = "arm") {
    if cfg!(target_feature = "thumb-mode") {
        2
    } else {
        4
    }
} else if cfg!(target_arch = "riscv") {
    if cfg!(target_feature = "c") {
        2
    } else {
        4
    }
} else {
    1
};

/// Define a `fn` item actually representing a `static` variable.
///
/// # Notes regarding accessing the variable in `asm!`
///
/// The variable might actually be placed in an address that is slightly off
/// from what the defined `fn` item represents. For example, on Thumb targets,
/// the least-significant bit of the `fn` item's address is set to indicate that
/// it's a Thumb function. However, we are actually using it as a variable
/// storage, so the bit must be cleared before use.
///
/// The [`SymStaticExt`] trait's methods automatically take care of such
/// situations. When referencing it from assembler code, **you must append `_`
/// to the symbol name** (see the following example).
///
/// # Examples
///
/// ```
/// #![feature(asm)]
/// #![feature(asm_sym)]
/// #![feature(asm_const)]
/// #![feature(naked_functions)]
/// use r3_portkit::sym::{sym_static, SymStatic, SymStaticExt};
/// use std::cell::Cell;
///
/// struct InteriorMutable(Cell<usize>);
/// unsafe impl Sync for InteriorMutable {}
/// unsafe impl r3::utils::ZeroInit for InteriorMutable {}
///
/// trait Tr {
///     sym_static!(static VAR: SymStatic<InteriorMutable> = zeroed!());
/// }
///
/// impl Tr for u8 {}
/// impl Tr for u16 {}
/// impl Tr for &'_ u8 {}
/// impl<T> Tr for (T,) {}
///
/// fn main() {
///     // TODO: Replace with the following when const arguments become
///     //       inferrable:
///     //
///     //       let sr1: SymStatic<InteriorMutable, _> = u8::VAR;
///     let sr1 = sym_static(u8::VAR);
///     sr1.as_ref().0.set(42);
///     assert_eq!(sr1.as_ref().0.get(), 42);
///
///     // Each instantiation gets a unique storage.
///     let sr2 = sym_static(u16::VAR);
///     assert_eq!(sr2.as_ref().0.get(), 0);
///     sr2.as_ref().0.set(84);
///     assert_eq!(sr1.as_ref().0.get(), 42);
///     assert_eq!(sr2.as_ref().0.get(), 84);
///
///     // ...however, types only differing by lifetime parameters do not
///     // get a unique storage.
///     fn inner<'a>(_: &'a mut u8) {
///         let sr1 = sym_static(<&'static u8>::VAR);
///         let sr2 = sym_static(<&'a u8>::VAR);
///         sr1.as_ref().0.set(1);
///         sr2.as_ref().0.set(2);
///         assert_eq!(sr1.as_ref().0.get(), 2);
///         assert_eq!(sr2.as_ref().0.get(), 2);
///     }
///     inner(&mut 42);
///
///     // Since it appears as a `fn` item, it can be passed to `asm!` by a
///     // `sym` input.
///     let got_value: usize;
///     #[cfg(target_arch = "x86_64")]
///     #[cfg(target_os = "linux")]
///     unsafe {
///         asm!("
///             mov {0}, qword ptr [rip + {1}_@GOTPCREL]
///             mov {0}, qword ptr [{0}]",
///             out(reg) got_value, sym u8::VAR,
///         );
///     }
///     #[cfg(target_arch = "x86_64")]
///     #[cfg(not(target_os = "linux"))]
///     unsafe { asm!("mov {}, qword ptr [rip + {}_]", out(reg) got_value, sym u8::VAR) };
///     #[cfg(target_arch = "aarch64")]
///     #[cfg(target_os = "macos")]
///     unsafe {
///         asm!("
///             adrp {0}, {1}_@PAGE
///             ldr {0}, [{0}, {1}_@PAGEOFF]
///             ",
///             out(reg) got_value, sym u8::VAR
///         );
///     }
///     #[cfg(target_arch = "aarch64")]
///     #[cfg(not(target_os = "macos"))]
///     unsafe {
///         asm!("
///             adrp {0}, :got:{1}_
///             ldr {0}, [{0}, #:got_lo12:{1}_]
///             ldr {0}, [{0}]
///             ",
///             out(reg) got_value, sym u8::VAR
///         )
///     };
///     assert_eq!(got_value, 42);
/// }
/// ```
pub macro sym_static {
    (
        // A trait item
        $(#[$meta:meta])*
        $vis:vis static $name:ident: SymStatic<$ty:ty>
    ) => {
        $(#[$meta])*
        #[allow(non_snake_case)]
        $vis unsafe extern "C" fn $name(_: &'static $crate::sym::__UnsafeSymStaticMarker<$ty>) -> !;
    },
    (
        // A concrete definition
        $(#[$meta:meta])*
        $vis:vis static $name:ident: SymStatic<$ty:ty> = zeroed!()
    ) =>{
        $(#[$meta])*
        #[naked]
        // For some reason, the compiler generates a `ud2` instruction depsite
        // this being a naked function. The existence of an instruction in a
        // `.bss` section causes a compile error, so this, uh, function needs to
        // be placed in `.data` instead.
        #[cfg_attr(
            any(target_os = "macos", target_os = "ios"),
            link_section = "__DATA,__data"
        )]
        #[cfg_attr(
            not(any(target_os = "macos", target_os = "ios")),
            link_section = ".data"
        )]
        // FIXME: Add `#[repr(align(...))]` to this function to generate an aligned
        //        address in the first place. This attribute is being implemented by
        //        the following PR: <https://github.com/rust-lang/rust/pull/81234>
        //        The const parameter of `__UnsafeSymStaticMarker` takes this
        //        function's alignment.
        //
        #[allow(non_snake_case)]
        $vis unsafe extern "C" fn $name(_: &'static $crate::sym::__UnsafeSymStaticMarker<$ty, {$crate::sym::DEFAULT_FN_ALIGN}>) -> ! {
            #[allow(unused_unsafe)]
            unsafe {
                $crate::sym::asm!(
                    "
                        .p2align {1}
                        .global {0}_
                        {0}_:
                        .zero {2}
                    ",
                    sym Self::$name,
                    const $crate::sym::mem::align_of::<$ty>().trailing_zeros(),
                    const $crate::sym::mem::size_of::<$ty>(),
                    options(noreturn),
                );
            }
        }
    },
}
