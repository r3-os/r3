/// Branch predictor invalidate all
pub const BPIALL: BPIALLAccessor = BPIALLAccessor;
pub struct BPIALLAccessor;

impl register::cpu::RegisterWriteOnly<u32, ()> for BPIALLAccessor {
    sys_coproc_write_raw!(u32, [p15, c7, 0, c5, 6]);
}
