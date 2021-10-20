/// Data cache invalidate by set/way
pub const DCISW: DCISWAccessor = DCISWAccessor;
pub struct DCISWAccessor;

impl tock_registers::interfaces::Writeable for DCISWAccessor {
    type T = u32;
    type R = ();
    sys_coproc_write_raw!(u32, [p15, c7, 0, c6, 2]);
}
