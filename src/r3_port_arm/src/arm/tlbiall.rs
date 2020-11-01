/// Invalidate entire unified TLB
pub const TLBIALL: TLBIALLAccessor = TLBIALLAccessor;
pub struct TLBIALLAccessor;

impl register::cpu::RegisterWriteOnly<u32, ()> for TLBIALLAccessor {
    sys_coproc_write_raw!(u32, [p15, c7, 0, c5, 0]);
}
