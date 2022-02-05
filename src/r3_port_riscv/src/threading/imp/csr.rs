//! A RISC-V CSR accessor that implements an elaborate, systematic work-around
//! for the "unconstrained generic constant" error.
//!
//! In the current compiler, when constant values are derived from generic
//! parameters, trait bounds must be used to declare in the signature that the
//! callers are responsible for upholding the validity of constant value
//! derivation. For example:
//!
//! ```rust,compile_fail
//! #![feature(generic_const_exprs)]
//! #![feature(asm_const)]
//! fn hoge<const PRIVILEGE_LEVEL: usize>() {
//!     unsafe { core::arch::asm!("# {}", const PRIVILEGE_LEVEL + 1) };
//! }
//! ```
//!
//! The above code doesn't compile because the evaluation of `PRIVILEGE_LEVEL +
//! 1` may fail. This can be fixed by mentioning this expression in trait
//! bounds:
//!
//! ```rust
//! #![feature(generic_const_exprs)]
//! #![feature(asm_const)]
//! fn hoge<const PRIVILEGE_LEVEL: usize>()
//! where
//!     [(); { PRIVILEGE_LEVEL + 1 }]:,
//! {
//!     unsafe { core::arch::asm!("# {}", const PRIVILEGE_LEVEL + 1) };
//! }
//! ```
//!
//! Like usual trait bounds, these curious trait bounds are infectious - they
//! have to be added to all indirect callers as well. But there's one
//! significant difference between purely-type-based bounds and the others:
//! the latter kind of trait bounds can't be implied by any other ones, which
//! means there exists no way to group some of them and give them an alias.
//! This has a far-reaching impact in this crate's code, where many functions
//! are interconnected together, and many `const` operands are used. These trait
//! bounds are highly detrimental to readability and maintainability.
//!
//! The solution used here is twofold:
//!
//! (1) *Contain const-genericity in small scopes.* If CSR numbers were derived
//! from associated constants (as in `Traits::PRIVILEGE_LEVEL as usize *
//! 0x100`), they would have to be included in infectious trait bounds (e.g.,
//! `where [(): { Traits::PRIVILEGE_LEVEL as usize * 0x100 }]:`) splattered
//! everywhere. Instead, we have a trait named `CsrSetAccess`, providing
//! non-generic accessor methods for all required CSRs. Its trait implementation
//! is still riddled with that kind of trait bounds, but they can be generated
//! automatically by a macro. They don't "infect" anything because this type's
//! genericity "collapses" in the `use_port!` macro expansion.
//!
//! (2) The previous solution doesn't work for large inline assembly blocks
//! containing numerous references to CSRs. This brings us to the second
//! solution:
//! *Import constants as `sym` operands*. Interestingly, the `#[export_name]`
//! attributes can be used to give symbol names entirely comprised of numbers,
//! and when such symbols are used in `sym` operands, they are indeed
//! interpreted as the intended numbers. The catch is that we have to specify
//! `#[export_name]` for each possible value (i.e., we can't easily cover the
//! whole range of `usize`).
use core::arch::asm;
use unstringify::unstringify;

#[derive(Clone, Copy)]
pub struct Csr<const NUM: usize>;

/// A trait for types representing a specific CSR and providing access to it.
///
/// This trait is an implementation detail of this crate.
pub trait CsrAccessor: Copy {
    const NUM: usize;

    fn read(&self) -> usize;
    fn set(&self, value: usize);
    fn set_i<const VALUE: usize>(&self);
    fn clear(&self, value: usize);
    fn clear_i<const VALUE: usize>(&self);
    fn fetch_clear(&self, value: usize) -> usize;
    fn fetch_clear_i<const VALUE: usize>(&self) -> usize;
}

impl<const NUM: usize> CsrAccessor for Csr<NUM> {
    const NUM: usize = NUM;

    #[inline(always)]
    fn read(&self) -> usize {
        let read: usize;
        unsafe { asm!("csrr {read}, {NUM}", read = lateout(reg) read, NUM = const NUM) };
        read
    }

    #[inline(always)]
    fn set(&self, value: usize) {
        unsafe { asm!("csrs {NUM}, {value}", NUM = const NUM, value = in(reg) value) };
    }

    #[inline(always)]
    fn set_i<const VALUE: usize>(&self) {
        unsafe { asm!("csrsi {NUM}, {VALUE}", NUM = const NUM, VALUE = const VALUE) };
    }

    #[inline(always)]
    fn clear(&self, value: usize) {
        unsafe { asm!("csrc {NUM}, {value}", NUM = const NUM, value = in(reg) value) };
    }

    #[inline(always)]
    fn clear_i<const VALUE: usize>(&self) {
        unsafe { asm!("csrci {NUM}, {VALUE}", NUM = const NUM, VALUE = const VALUE) };
    }

    #[inline(always)]
    fn fetch_clear(&self, value: usize) -> usize {
        let read: usize;
        unsafe {
            asm!(
                "csrrc {read}, {NUM}, {value}",
                NUM = const NUM,
                read = lateout(reg) read,
                value = in(reg) value
            )
        };
        read
    }

    #[inline(always)]
    fn fetch_clear_i<const VALUE: usize>(&self) -> usize {
        let read: usize;
        unsafe {
            asm!(
                "csrrci {read}, {NUM}, {VALUE}",
                read = lateout(reg) read,
                NUM = const NUM,
                VALUE = const VALUE
            )
        };
        read
    }
}

/// Implements [`CsrSetAccess`].
///
/// This type is an implementation detail of this crate.
pub struct CsrSet<Traits>(Traits);

macro_rules! define_set {
    (
        impl<Traits: super::ThreadingOptions> CsrSet<Traits> {}

        $( #[$csrexpr_meta:meta] )*
        macro csrexpr {
            $(
                $( #[$const_meta:meta] )*
                ($CONST:ident) => { $const_value:literal }
            ),*
            $(,)?
        }

        impl<Traits> CsrSetAccess for CsrSet<Traits> {
            $(
                #[csr_accessor(ty = $Csr:ident)]
                $( #[$csr_meta:meta] )*
                fn $csr:ident() -> Csr<{ $offset:expr }>;
            )*

            $(
                #[csr_immediate]
                $( #[$csr_immediate_meta:meta] )*
                fn $csr_immediate:ident() $( -> $csr_immediate_ret:ty )? {
                    Self::$csr_immediate_target:ident()
                        .$csr_immediate_op:ident
                        ::<$({ $csr_immediate_param:expr }),*>()
                }
            )*
        }
    ) => {
        impl<Traits: super::ThreadingOptions> CsrSet<Traits> {
            const PRIV: usize = {
                assert!(Traits::PRIVILEGE_LEVEL < 4, "`PRIVILEGE_LEVEL` must be in the range `0..4`");
                Traits::PRIVILEGE_LEVEL as usize
            };

            $(
                pub const $CONST: usize = {
                    #[allow(non_snake_case)]
                    let PRIV = Self::PRIV; // provide a value for `{PRIV}` in `$const_value`
                    unstringify!($const_value)
                };
            )*
        }

        $( #[$csrexpr_meta] )*
        pub(crate) macro csrexpr {
            $(
                ($CONST) => { $const_value }
            ),*
        }

        /// Provides all CSR accessors needed by this crate's port implementation.
        ///
        /// This trait is an implementation detail of this crate.
        pub trait CsrSetAccess {
            $(
                $( #[$const_meta] )*
                const $CONST: usize;
            )*

            $(
                $( #[$csr_meta] )*
                type $Csr: CsrAccessor;

                $( #[$csr_meta] )*
                fn $csr() -> Self::$Csr;
            )*

            $(
                $( #[$csr_immediate_meta] )*
                fn $csr_immediate() $( -> $csr_immediate_ret )?;
            )*
        }

        impl<Traits> CsrSetAccess for CsrSet<Traits>
        where
            Traits: super::ThreadingOptions,
            $( [(); { $offset }]:, )*
            $($( [(); { $csr_immediate_param }]:, )*)*
        {
            $(
                const $CONST: usize = Self::$CONST;
            )*

            $(
                type $Csr = Csr<{ $offset }>;

                #[inline(always)]
                fn $csr() -> Csr<{ $offset }> { Csr::<{ $offset }> }
            )*

            $(
                #[inline(always)]
                fn $csr_immediate() $( -> $csr_immediate_ret )? {
                    Self::$csr_immediate_target()
                        .$csr_immediate_op
                        ::<$({ $csr_immediate_param }),*>()
                }
            )*
        }
    }
}

pub const XSTATUS_MPP_M: usize = 0b11 << 11;
pub const XSTATUS_SPP_S: usize = 1 << 8;
pub const XSTATUS_FS_0: usize = 1 << 13;
pub const XSTATUS_FS_1: usize = 1 << 14;

pub const XCAUSE_INTERRUPT: usize = usize::MAX - usize::MAX / 2;
pub const XCAUSE_EXCEPTIONCODE_MASK: usize = usize::MAX / 2;

define_set! {
    impl<Traits: super::ThreadingOptions> CsrSet<Traits> {
        /* `csrexpr!` is also exposed as `const`s here */
    }

    /// Create an assembly expression that evaluates to a CSR number or value.
    /// Assumes the presence of an operand `PRIV = sym Traits::Priv::value`.
    macro csrexpr {
        // CSRs
        (XSTATUS) => { "{PRIV} * 0x100" },
        (XIE) => { "{PRIV} * 0x100 + 0x04" },
        (XEPC) => { "{PRIV} * 0x100 + 0x41" },
        (XCAUSE) => { "{PRIV} * 0x100 + 0x42" },
        (XIP) => { "{PRIV} * 0x100 + 0x44" },

        // CSR values
        // Machine/Supervisor/... Interrupt Enable
        (XSTATUS_XIE) =>  { "1 << ({PRIV})" },
        // Machine/Supervisor/... Previous Interrupt Enable
        (XSTATUS_XPIE) =>  { "1 << ({PRIV} + 4)" },

        /// Machine/Supervisor/... Software Interrupt Enable
        (XIE_XSIE) =>  { "1 << ({PRIV})" },
        /// Machine/Supervisor/... Timer Interrupt Enable
        (XIE_XTIE) =>  { "1 << ({PRIV} + 4)" },
        /// Machine/Supervisor/... External Interrupt Enable
        (XIE_XEIE) =>  { "1 << ({PRIV} + 8)" },

        /// Machine/Supervisor/... Software Interrupt Pending
        (XIP_XSIP) =>  { "1 << ({PRIV})" },
        /// Machine/Supervisor/... Timer Interrupt Pending
        (XIP_XTIP) =>  { "1 << ({PRIV} + 4)" },
        /// Machine/Supervisor/... External Interrupt Pending
        (XIP_XEIP) =>  { "1 << ({PRIV} + 8)" },
    }

    impl<Traits> CsrSetAccess for CsrSet<Traits> {
        #[csr_accessor(ty = Xstatus)]
        /// `λstatus` (Machine/Supervisor/... Status Register)
        fn xstatus() -> Csr<{ Self::XSTATUS }>;

        #[csr_accessor(ty = Xie)]
        /// `λie` (Machine/Supervisor/... Interrupt Enable)
        fn xie() -> Csr<{ Self::XIE }>;

        #[csr_accessor(ty = Xcause)]
        /// `λcause` (Machine/Supervisor/... Cause Register)
        fn xcause() -> Csr<{ Self::XCAUSE }>;

        #[csr_accessor(ty = Xip)]
        /// `λip` (Machine/Supervisor/... Interrupt Pending)
        fn xip() -> Csr<{ Self::XIP }>;

        #[csr_immediate]
        /// Set `λstatus.λIE`.
        fn xstatus_set_xie() {
            Self::xstatus().set_i::<{ Self::XSTATUS_XIE }>()
        }

        #[csr_immediate]
        /// Clear `λstatus.λIE`.
        fn xstatus_clear_xie() {
            Self::xstatus().clear_i::<{ Self::XSTATUS_XIE }>()
        }

        #[csr_immediate]
        /// Clear `λstatus.λIE`, returning the original value of this CSR.
        fn xstatus_fetch_clear_xie() -> usize {
            Self::xstatus().fetch_clear_i::<{ Self::XSTATUS_XIE }>()
        }
    }
}

/// An integer, exposed as a symbol name
pub trait Num {
    fn value();
}

/// `<NumTy<N> as Num>::value` has a symbol name `N`.
pub struct NumTy<const N: usize>;

seq_macro::seq!(N in 0..4 {
    #[doc(hidden)]
    impl Num for NumTy<N> {
        #[export_name = stringify!(N)]
        fn value() {}
    }
});
