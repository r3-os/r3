/// Data cache invalidate by set/way
pub const DCISW: DCISWAccessor = DCISWAccessor;
pub struct DCISWAccessor;

impl register::cpu::RegisterWriteOnly<u32, ()> for DCISWAccessor {
    sys_coproc_write_raw!(u32, [p15, c7, 0, c6, 2]);
}
