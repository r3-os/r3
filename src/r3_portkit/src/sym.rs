//! Simulates associated `static`s in traits.
// FIXME: Remove this module when real `static`s are supported in traits
use core::marker::PhantomData;
use r3::utils::ZeroInit;

/// Used as a parameter for the [`SymStatic`] function pointer type.
///
/// An external crate must not name this type. Defining a function with
/// a signature matching [`SymStatic`] is unsafe.
#[repr(C)]
#[doc(hidden)]
pub struct __UnsafeSymStaticMarker<T: 'static>(PhantomData<&'static T>);

#[doc(hidden)]
pub use core::{asm, mem};

/// Represents a value of `&'static T`. The functions of this type are defined
/// by [`sym_static!`].
pub type SymStatic<T> = unsafe extern "C" fn(&'static __UnsafeSymStaticMarker<T>) -> !;

/// Coerce a value into [`SymStatic`].
pub const fn sym_static<T: 'static>(x: SymStatic<T>) -> SymStatic<T> {
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

unsafe impl<T> SymStaticExt for SymStatic<T> {
    type Output = T;

    #[inline(always)]
    #[cfg(all(target_arch = "arm", target_feature = "thumb-mode"))]
    fn as_ptr(self) -> *const T {
        // Remove the Thumb flag. Unfortunately, the current compiler does not
        // fold the offset into the symbol reference, so this will end up in
        // instructions like the following:
        //
        //   ldr        r0, =0x20000001
        //   subs       r0, r0, #0x1
        //
        (self as usize - 1) as *const T
    }

    #[inline(always)]
    #[cfg(not(all(target_arch = "arm", target_feature = "thumb-mode")))]
    fn as_ptr(self) -> *const T {
        self as usize as *const T
    }
}

/// Define a `fn` item actually representing a `static` variable.
///
/// # Target-specific Notes
///
/// *On Thumb targets*, the least-significant bit of the `fn` item's address is
/// set to indicate that it's a Thumb function. However, we are actually using
/// it as a variable storage, so the bit must be cleared before use.
/// The [`SymStaticExt`] trait's methods automatically take care of this. When
/// referencing it from assembler code, you must append `_` to the symbol name.
///
/// # Examples
///
/// ```
/// #![feature(asm)]
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
///     let sr1: SymStatic<InteriorMutable> = u8::VAR;
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
///         let sr1: SymStatic<InteriorMutable> = <&'static u8>::VAR;
///         let sr2: SymStatic<InteriorMutable> = <&'a u8>::VAR;
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
///             mov {0}, qword ptr [rip + {1}@GOTPCREL]
///             mov {0}, qword ptr [{0}]",
///             out(reg) got_value, sym u8::VAR,
///         );
///     }
///     #[cfg(target_arch = "x86_64")]
///     #[cfg(not(target_os = "linux"))]
///     unsafe { asm!("mov {}, qword ptr [rip + {}]", out(reg) got_value, sym u8::VAR) };
///     #[cfg(target_arch = "aarch64")]
///     #[cfg(target_os = "macos")]
///     unsafe {
///         asm!("
///             adrp {0}, {1}@PAGE
///             ldr {0}, [{0}, {1}@PAGEOFF]
///             ",
///             out(reg) got_value, sym u8::VAR
///         );
///     }
///     #[cfg(target_arch = "aarch64")]
///     #[cfg(not(target_os = "macos"))]
///     unsafe {
///         asm!("
///             adrp {0}, :got:{1}
///             ldr {0}, [{0}, #:got_lo12:{1}]
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
        #[allow(non_snake_case)]
        $vis unsafe extern "C" fn $name(_: &'static $crate::sym::__UnsafeSymStaticMarker<$ty>) -> ! {
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
