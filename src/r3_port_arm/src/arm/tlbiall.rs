/// Invalidate entire unified TLB
pub const TLBIALL: TLBIALLAccessor = TLBIALLAccessor;
pub struct TLBIALLAccessor;

impl tock_registers::interfaces::Writeable for TLBIALLAccessor {
    type T = u32;
    type R = ();
    sys_coproc_write_raw!(u32, [p15, c7, 0, c5, 0]);
}
