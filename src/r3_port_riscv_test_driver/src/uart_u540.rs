//! The UART driver compatible with QEMU `sifive_u` machine
//! (RISC-V Board compatible with SiFive U SDK).
use core::fmt::{self, Write};

pub fn stdout_write_str(s: &str) {
    crate::with_cpu_lock(|| {
        let _ = SerialWrapper(0x10010000 as *mut u32).write_str(s);
    });
}

pub fn stdout_write_fmt(args: fmt::Arguments<'_>) {
    crate::with_cpu_lock(|| {
        let _ = SerialWrapper(0x10010000 as *mut u32).write_fmt(args);
    });
}

pub fn stderr_write_fmt(args: fmt::Arguments<'_>) {
    crate::with_cpu_lock(|| {
        let _ = SerialWrapper(0x10011000 as *mut u32).write_fmt(args);
    });
}

struct SerialWrapper(*mut u32);

impl SerialWrapper {
    fn write_u8(&self, x: u8) {
        // On QEMU, this will instantly send the character to the console
        unsafe { self.0.write_volatile(x as _) };
    }
}

impl Write for SerialWrapper {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.as_bytes() {
            if *byte == '\n' as u8 {
                self.write_u8('\r' as u8);
            }

            self.write_u8(*byte);
        }
        Ok(())
    }
}
