//! CPU scheduling analysis
use constance::kernel::{cfg::CfgBuilder, Kernel};
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

const I_LW_SERIAL: Interval = "load u32 (+ add) serial x 1000";
const I_LW_PARALLEL: Interval = "load u32 parallel x 1000";
const I_LH_SERIAL: Interval = "load u16 (+ add) serial x 1000";
const I_LH_PARALLEL: Interval = "load u16 parallel x 1000";

impl<System: Kernel> AppInner<System> {
    /// Used by `use_benchmark_in_kernel_benchmark!`
    const fn new<B: Bencher<Self>>(b: &mut CfgBuilder<System>) -> Self {
        Self {
            _phantom: PhantomData,
        }
    }

    /// Used by `use_benchmark_in_kernel_benchmark!`
    fn iter<B: Bencher<Self>>() {
        #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
        unsafe {
            let mut dummy = 0u32;
            let mut var = (&mut dummy) as *mut _;

            B::mark_start();
            for _ in 0..100 {
                asm!("
                    lw {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lw {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lw {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lw {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lw {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lw {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lw {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lw {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lw {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lw {tmp}, ({r})
                    add {r}, {r}, {tmp}
                ",  r = inout(reg) var => var
                ,   tmp = out(reg) _);
            }
            B::mark_end(I_LW_SERIAL);
        }

        #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
        unsafe {
            let mut dummy = 42u32;
            let var = (&mut dummy) as *mut _;

            B::mark_start();
            for _ in 0..100 {
                asm!("
                    lw {out1}, ({r})
                    lw {out2}, ({r})
                    lw {out3}, ({r})
                    lw {out4}, ({r})
                    lw {out5}, ({r})
                    lw {out1}, ({r})
                    lw {out2}, ({r})
                    lw {out3}, ({r})
                    lw {out4}, ({r})
                    lw {out5}, ({r})
                ",  r = in(reg) var
                ,   out1 = out(reg) _
                ,   out2 = out(reg) _
                ,   out3 = out(reg) _
                ,   out4 = out(reg) _
                ,   out5 = out(reg) _);
            }
            B::mark_end(I_LW_PARALLEL);
        }

        #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
        unsafe {
            let mut dummy = 0;
            let mut var = (&mut dummy) as *mut _;

            B::mark_start();
            for _ in 0..100 {
                asm!("
                    lh {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lh {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lh {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lh {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lh {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lh {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lh {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lh {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lh {tmp}, ({r})
                    add {r}, {r}, {tmp}
                    lh {tmp}, ({r})
                    add {r}, {r}, {tmp}
                ",  r = inout(reg) var => var
                ,   tmp = out(reg) _);
            }
            B::mark_end(I_LH_SERIAL);
        }

        #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
        unsafe {
            let mut dummy = 42u16;
            let var = (&mut dummy) as *mut _;

            B::mark_start();
            for _ in 0..100 {
                asm!("
                    lh {out1}, ({r})
                    lh {out2}, ({r})
                    lh {out3}, ({r})
                    lh {out4}, ({r})
                    lh {out5}, ({r})
                    lh {out1}, ({r})
                    lh {out2}, ({r})
                    lh {out3}, ({r})
                    lh {out4}, ({r})
                    lh {out5}, ({r})
                ",  r = in(reg) var
                ,   out1 = out(reg) _
                ,   out2 = out(reg) _
                ,   out3 = out(reg) _
                ,   out4 = out(reg) _
                ,   out5 = out(reg) _);
            }
            B::mark_end(I_LH_PARALLEL);
        }
    }
}
