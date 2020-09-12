pub const MTIME_FREQUENCY: u64 = 10_000_000;

#[export_name = "_mp_hook"]
pub fn mp_hook() -> bool {
    match riscv::register::mhartid::read() {
        0 => unsafe {
            // Wake up the RV32GC core (hartid = 1)
            (0x200_0004 as *mut u32).write_volatile(1);
        },
        1 => return true,
        _ => {}
    }
    loop {
        unsafe { asm!("wfi") };
    }
}
