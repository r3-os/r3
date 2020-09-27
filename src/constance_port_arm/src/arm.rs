/// Implements `register::cpu::RegisterReadWrite::get`.
macro_rules! sys_coproc_read_raw {
    ($width:ty, [$cp:ident, $crn:ident, $opc1:literal, $crm:ident, $opc2:literal]) => {
        #[inline]
        fn get(&self) -> u32 {
            let reg;
            unsafe {
                asm!(
                    concat!(
                        "mrc ", stringify!($cp), ", ", stringify!($opc1), ", {}, ",
                        stringify!($crn), ", ", stringify!($crm), ", ", stringify!($opc2)
                    ),
                    lateout(reg) reg,
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
                asm!(
                    concat!(
                        "mcr ", stringify!($cp), ", ", stringify!($opc1), ", {}, ",
                        stringify!($crn), ", ", stringify!($crm), ", ", stringify!($opc2)
                    ),
                    in(reg) value,
                );
            }
        }
    };
}

mod bpiall;
mod ccsidr;
mod clidr;
mod csselr;
mod dacr;
mod dcisw;
mod iciallu;
mod sctlr;
mod tlbiall;
mod ttbcr;
mod ttbr0;
pub use self::bpiall::*;
pub use self::ccsidr::*;
pub use self::clidr::*;
pub use self::csselr::*;
pub use self::dacr::*;
pub use self::dcisw::*;
pub use self::iciallu::*;
pub use self::sctlr::*;
pub use self::tlbiall::*;
pub use self::ttbcr::*;
pub use self::ttbr0::*;
