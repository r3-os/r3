//! The implementation of the RISC-V timer driver.
use r3::kernel::{cfg::CfgBuilder, InterruptHandler, Kernel, PortToKernel, UTicks};
use r3_portkit::tickless::{TicklessCfg, TicklessStateTrait};
use tock_registers::{
    interfaces::{Readable, Writeable},
    registers::ReadWrite,
};

use crate::timer::cfg::TimerOptions;

/// Implemented on a system type by [`use_timer!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_timer!`].
pub unsafe trait TimerInstance: Kernel + TimerOptions {
    // FIXME: Specifying `TicklessCfg::new(...)` here causes a "cycle
    //        detected" error
    const TICKLESS_CFG: TicklessCfg;

    // FIXME: Specifying `TicklessState<{ Self::TICKLESS_CFG }>` here
    //        fails with an error message similar to
    //        <https://github.com/rust-lang/rust/issues/72821>
    type TicklessState: TicklessStateTrait;

    fn tickless_state() -> *mut Self::TicklessState;
}

trait TimerInstanceExt: TimerInstance {
    #[inline(always)]
    fn mtime_reg32() -> &'static [ReadWrite<u32>; 2] {
        // Safety: Verified by the user of `use_timer!`
        unsafe { &*(Self::MTIME_PTR as *const _) }
    }

    #[inline(always)]
    fn mtime_reg64() -> &'static ReadWrite<u64> {
        // Safety: Verified by the user of `use_timer!`
        unsafe { &*(Self::MTIME_PTR as *const _) }
    }

    #[inline(always)]
    fn mtimecmp_reg32() -> &'static [ReadWrite<u32>; 2] {
        // Safety: Verified by the user of `use_timer!`
        unsafe { &*(Self::MTIMECMP_PTR as *const _) }
    }

    #[cfg(target_arch = "riscv64")]
    #[inline(always)]
    fn mtime() -> u64 {
        Self::mtime_reg64().get()
    }

    #[cfg(not(target_arch = "riscv64"))]
    #[inline(always)]
    fn mtime() -> u64 {
        loop {
            let hi1 = Self::mtime_reg32()[1].get();
            let lo = Self::mtime_reg32()[0].get();
            let hi2 = Self::mtime_reg32()[1].get();
            if hi1 == hi2 {
                return lo as u64 | ((hi2 as u64) << 32);
            }
        }
    }
}
impl<T: TimerInstance> TimerInstanceExt for T {}

/// The configuration function.
pub const fn configure<System: TimerInstance>(b: &mut CfgBuilder<System>) {
    InterruptHandler::build()
        .line(System::INTERRUPT_NUM)
        .start(handle_tick::<System>)
        .finish(b);
}

/// Implements [`crate::Timer::init`]
#[inline]
pub fn init<System: TimerInstance>() {
    let tcfg = &System::TICKLESS_CFG;

    // Safety: No context switching during boot
    let tstate = unsafe { &mut *System::tickless_state() };

    if System::RESET_MTIME {
        System::mtime_reg32()[0].set(0);
    } else {
        tstate.reset(tcfg, System::mtime_reg32()[0].get());
    }
}

/// Implements [`r3::kernel::PortTimer::tick_count`]
///
/// # Safety
///
/// Only meant to be referenced by `use_timer!`.
pub unsafe fn tick_count<System: TimerInstance>() -> UTicks {
    let tcfg = &System::TICKLESS_CFG;

    let hw_tick_count = System::mtime_reg32()[0].get();

    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *System::tickless_state() };
    tstate.tick_count(tcfg, hw_tick_count)
}

/// Implements [`r3::kernel::PortTimer::pend_tick`]
///
/// # Safety
///
/// Only meant to be referenced by `use_timer!`.
pub unsafe fn pend_tick<System: TimerInstance>() {
    System::mtimecmp_reg32()[0].set(0);
    System::mtimecmp_reg32()[1].set(0);
}

/// Implements [`r3::kernel::PortTimer::pend_tick_after`]
///
/// # Safety
///
/// Only meant to be referenced by `use_timer!`.
pub unsafe fn pend_tick_after<System: TimerInstance>(tick_count_delta: UTicks) {
    let tcfg = &System::TICKLESS_CFG;
    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *System::tickless_state() };

    let cur_hw_tick_count = System::mtime();
    let hw_ticks = tstate
        .mark_reference_and_measure(tcfg, cur_hw_tick_count as u32, tick_count_delta)
        .hw_ticks;

    let next_hw_tick_count = cur_hw_tick_count + hw_ticks as u64;

    // Since we have CPU Lock, spurious timer interrupts while non-atomically
    // updating `mtimecmp` are acceptable
    System::mtimecmp_reg32()[0].set(next_hw_tick_count as u32);
    System::mtimecmp_reg32()[1].set((next_hw_tick_count >> 32) as u32);
}

#[inline]
fn handle_tick<System: TimerInstance>(_: usize) {
    let tcfg = &System::TICKLESS_CFG;

    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *System::tickless_state() };

    let cur_hw_tick_count = System::mtime_reg32()[0].get();
    tstate.mark_reference(tcfg, cur_hw_tick_count);

    // Safety: CPU Lock inactive, an interrupt context
    unsafe { System::timer_tick() };
}
