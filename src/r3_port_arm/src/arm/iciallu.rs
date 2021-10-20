/// Instruction cache invalidate all
pub const ICIALLU: ICIALLUAccessor = ICIALLUAccessor;
pub struct ICIALLUAccessor;

impl tock_registers::interfaces::Writeable for ICIALLUAccessor {
    type T = u32;
    type R = ();
    sys_coproc_write_raw!(u32, [p15, c7, 0, c5, 0]);
}
