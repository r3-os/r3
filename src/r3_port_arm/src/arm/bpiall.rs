/// Branch predictor invalidate all
pub const BPIALL: BPIALLAccessor = BPIALLAccessor;
pub struct BPIALLAccessor;

impl tock_registers::interfaces::Writeable for BPIALLAccessor {
    type T = u32;
    type R = ();
    sys_coproc_write_raw!(u32, [p15, c7, 0, c5, 6]);
}
