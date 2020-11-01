/// Instruction cache invalidate all
pub const ICIALLU: ICIALLUAccessor = ICIALLUAccessor;
pub struct ICIALLUAccessor;

impl register::cpu::RegisterWriteOnly<u32, ()> for ICIALLUAccessor {
    sys_coproc_write_raw!(u32, [p15, c7, 0, c5, 0]);
}
