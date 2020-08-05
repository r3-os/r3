/// Implements `register::cpu::RegisterReadWrite::get`.
macro_rules! sys_coproc_read_raw {
    ($width:ty, [$cp:ident, $crn:ident, $opc1:literal, $crm:ident, $opc2:literal]) => {
        #[inline]
        fn get(&self) -> u32 {
            let reg;
            unsafe {
                llvm_asm!(
                    concat!(
                        "mrc ", stringify!($cp), ", ", stringify!($opc1), ", $0, ",
                        stringify!($crn), ", ", stringify!($crm), ", ", stringify!($opc2)
                    )
                :   "=r"(reg)
                :
                :
                :   "volatile"
                );
            }
            reg
        }
    };
}
/// Implements `register::cpu::RegisterReadWrite::set`.
macro_rules! sys_coproc_write_raw {
    ($width:ty, [$cp:ident, $crn:ident, $opc1:literal, $crm:ident, $opc2:literal]) => {
        #[inline]
        fn set(&self, value: u32) {
            unsafe {
                llvm_asm!(
                    concat!(
                        "mcr ", stringify!($cp), ", ", stringify!($opc1), ", $0, ",
                        stringify!($crn), ", ", stringify!($crm), ", ", stringify!($opc2)
                    )
                :
                :   "r"(value)
                :
                :   "volatile"
                );
            }
        }
    };
}

mod dacr;
mod iciallu;
mod sctlr;
mod tlbiall;
mod ttbcr;
mod ttbr0;
pub use self::dacr::*;
pub use self::iciallu::*;
pub use self::sctlr::*;
pub use self::tlbiall::*;
pub use self::ttbcr::*;
pub use self::ttbr0::*;
