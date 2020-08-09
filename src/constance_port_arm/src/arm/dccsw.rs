/// Data cache clean by set/way
pub const DCCSW: DCCSWAccessor = DCCSWAccessor;
pub struct DCCSWAccessor;

impl register::cpu::RegisterWriteOnly<u32, ()> for DCCSWAccessor {
    sys_coproc_write_raw!(u32, [p15, c7, 0, c10, 2]);
}
