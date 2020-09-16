//! CPU scheduling analysis
use constance::kernel::{cfg::CfgBuilder, Kernel, StartupHook};
use core::marker::PhantomData;

use super::Bencher;
use crate::utils::benchmark::Interval;

use_benchmark_in_kernel_benchmark! {
    pub unsafe struct App<System> {
        inner: AppInner<System>,
    }
}

struct AppInner<System> {
    _phantom: PhantomData<System>,
}

const I_ADD_SERIAL: Interval = "add serial x 1000";
const I_ADD_PARALLEL: Interval = "add parallel x 1000";
const I_MUL_SERIAL: Interval = "mul serial x 1000";
const I_MUL_PARALLEL: Interval = "mul parallel x 1000";
const I_DIV1_PARALLEL: Interval = "div parallel 1 x 1000";
const I_DIV2_PARALLEL: Interval = "div parallel 2 x 1000";

impl<System: Kernel> AppInner<System> {
    /// Used by `use_benchmark_in_kernel_benchmark!`
    const fn new<B: Bencher<Self>>(b: &mut CfgBuilder<System>) -> Self {
        StartupHook::build().start(run_once).finish(b);

        Self {
            _phantom: PhantomData,
        }
    }

    /// Used by `use_benchmark_in_kernel_benchmark!`
    fn iter<B: Bencher<Self>>() {
        #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
        unsafe {
            B::mark_start();
            for _ in 0..100 {
                asm!("
                    add {r}, {r}, {r}
                    add {r}, {r}, {r}
                    add {r}, {r}, {r}
                    add {r}, {r}, {r}
                    add {r}, {r}, {r}
                    add {r}, {r}, {r}
                    add {r}, {r}, {r}
                    add {r}, {r}, {r}
                    add {r}, {r}, {r}
                    add {r}, {r}, {r}
                ",  r = out(reg) _);
            }
            B::mark_end(I_ADD_SERIAL);
        }

        #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
        unsafe {
            B::mark_start();
            for _ in 0..100 {
                asm!("
                    add {r}, {in1}, {in2}
                    add {r}, {in1}, {in2}
                    add {r}, {in1}, {in2}
                    add {r}, {in1}, {in2}
                    add {r}, {in1}, {in2}
                    add {r}, {in1}, {in2}
                    add {r}, {in1}, {in2}
                    add {r}, {in1}, {in2}
                    add {r}, {in1}, {in2}
                    add {r}, {in1}, {in2}
                ",  r = out(reg) _
                ,   in1 = out(reg) _
                ,   in2 = out(reg) _);
            }
            B::mark_end(I_ADD_PARALLEL);
        }

        #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
        unsafe {
            let mut var = 42;
            B::mark_start();
            for _ in 0..100 {
                asm!("
                    mul {r}, {r}, {r}
                    mul {r}, {r}, {r}
                    mul {r}, {r}, {r}
                    mul {r}, {r}, {r}
                    mul {r}, {r}, {r}
                    mul {r}, {r}, {r}
                    mul {r}, {r}, {r}
                    mul {r}, {r}, {r}
                    mul {r}, {r}, {r}
                    mul {r}, {r}, {r}
                ",  r = inout(reg) var => var);
            }
            B::mark_end(I_MUL_SERIAL);
        }

        #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
        unsafe {
            let mut var1 = 0xdeadbeefu32;
            let mut var2 = 0xdeadbeeeu32;
            B::mark_start();
            for _ in 0..100 {
                asm!("
                    mul {out1}, {in1}, {in2}
                    mul {out2}, {in1}, {in2}
                    mul {out3}, {in1}, {in2}
                    mul {out4}, {in1}, {in2}
                    mul {out5}, {in1}, {in2}
                    mul {out1}, {in1}, {in2}
                    mul {out2}, {in1}, {in2}
                    mul {out3}, {in1}, {in2}
                    mul {out4}, {in1}, {in2}
                    mul {out5}, {in1}, {in2}
                ",  out1 = out(reg) _
                ,   out2 = out(reg) _
                ,   out3 = out(reg) _
                ,   out4 = out(reg) _
                ,   out5 = out(reg) _
                ,   in1 = inout(reg) var1 => var1
                ,   in2 = inout(reg) var2 => var2);
            }
            B::mark_end(I_MUL_PARALLEL);
        }

        #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
        unsafe {
            let mut var1 = 0xdeadbeefu32;
            let mut var2 = 0xdeu32;
            B::mark_start();
            for _ in 0..100 {
                asm!("
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                ",  r = out(reg) _
                ,   in1 = inout(reg) var1 => var1
                ,   in2 = inout(reg) var2 => var2);
            }
            B::mark_end(I_DIV1_PARALLEL);
        }

        #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
        unsafe {
            let mut var1 = 0xdeadbeefu32;
            let mut var2 = 0xdeadbeeeu32;
            B::mark_start();
            for _ in 0..100 {
                asm!("
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                    div {r}, {in1}, {in2}
                ",  r = out(reg) _
                ,   in1 = inout(reg) var1 => var1
                ,   in2 = inout(reg) var2 => var2);
            }
            B::mark_end(I_DIV2_PARALLEL);
        }
    }
}

fn run_once(_: usize) {
    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    unsafe {
        let mvendorid: usize;
        asm!("csrr {}, mvendorid", out(reg) mvendorid);
        log::warn!("mvendorid = 0x{:x}", mvendorid);

        let marchid: usize;
        asm!("csrr {}, marchid", out(reg) marchid);
        log::warn!("marchid = 0x{:x}", marchid);

        let mimpid: usize;
        asm!("csrr {}, mimpid", out(reg) mimpid);
        log::warn!("mimpid = 0x{:x}", mimpid);
    }
}
