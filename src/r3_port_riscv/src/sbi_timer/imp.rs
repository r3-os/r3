//! The implementation of the SBI-based timer driver.
use core::arch::asm;
use r3_core::kernel::{traits, Cfg, StaticInterruptHandler};
use r3_kernel::{KernelTraits, PortToKernel, System, UTicks};
use r3_portkit::tickless::{TicklessCfg, TicklessStateTrait};

use crate::sbi_timer::cfg::SbiTimerOptions;

/// Implemented on a kernel trait type by [`use_sbi_timer!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_sbi_timer!`].
pub unsafe trait TimerInstance: KernelTraits + SbiTimerOptions {
    // FIXME: Specifying `TicklessCfg::new(...)` here causes a "cycle
    //        detected" error
    const TICKLESS_CFG: TicklessCfg;

    // FIXME: Specifying `TicklessState<{ Self::TICKLESS_CFG }>` here
    //        fails with an error message similar to
    //        <https://github.com/rust-lang/rust/issues/72821>
    type TicklessState: TicklessStateTrait;

    fn tickless_state() -> *mut Self::TicklessState;
}

#[cfg(any(
    target_arch = "riscv32",
    target_arch = "riscv64",
    target_arch = "riscv128"
))]
trait TimerInstanceExt: TimerInstance {
    #[inline(always)]
    fn time_lo() -> usize {
        let read: usize;
        unsafe { asm!("csrr {read}, time", read = lateout(reg) read) };
        read
    }

    #[cfg(target_arch = "riscv32")]
    #[inline(always)]
    fn time_hi() -> usize {
        let read: usize;
        unsafe { asm!("csrr {read}, timeh", read = lateout(reg) read) };
        read
    }

    #[inline(always)]
    #[cfg(target_arch = "riscv32")]
    fn set_timecmp(value: u64) {
        unsafe {
            asm!(
                "ecall",
                inout("a0") value as u32 => _,  // param0 => error
                inout("a1") (value >> 32) as u32 => _, // param => value
                out("a2") _,
                out("a3") _,
                out("a4") _,
                out("a5") _,
                inout("a6") 0 => _, //fid
                inout("a7") 0x54494D45 => _, // eid
            )
        };
    }

    #[inline(always)]
    #[cfg(not(target_arch = "riscv32"))]
    fn set_timecmp(value: u64) {
        unsafe {
            asm!(
                "ecall",
                inout("a0") value as usize => _,  // param0 => error
                out("a1") _,
                out("a2") _,
                out("a3") _,
                out("a4") _,
                out("a5") _,
                inout("a6") 0 => _, //fid
                inout("a7") 0x54494D45 => _, // eid
            )
        };
    }

    #[cfg(not(target_arch = "riscv32"))]
    #[inline(always)]
    fn time() -> u64 {
        Self::time_lo() as u64
    }

    #[cfg(target_arch = "riscv32")]
    #[inline(always)]
    fn time() -> u64 {
        loop {
            let hi1 = Self::time_hi();
            let lo = Self::time_lo();
            let hi2 = Self::time_hi();
            if hi1 == hi2 {
                return lo as u64 | ((hi2 as u64) << 32);
            }
        }
    }
}
#[cfg(not(any(
    target_arch = "riscv32",
    target_arch = "riscv64",
    target_arch = "riscv128"
)))]
trait TimerInstanceExt: TimerInstance {
    fn time_lo() -> usize {
        unimplemented!("target mismatch")
    }

    fn set_timecmp(value: u64) {
        unimplemented!("target mismatch")
    }

    fn time() -> u64 {
        unimplemented!("target mismatch")
    }
}
impl<T: TimerInstance> TimerInstanceExt for T {}

/// The configuration function.
pub const fn configure<C, Traits: TimerInstance>(b: &mut Cfg<C>)
where
    C: ~const traits::CfgInterruptLine<System = System<Traits>>,
{
    StaticInterruptHandler::define()
        .line(Traits::INTERRUPT_NUM)
        .start(handle_tick::<Traits>)
        .finish(b);
}

/// Implements [`crate::Timer::init`]
#[inline]
pub fn init<Traits: TimerInstance>() {
    let tcfg = &Traits::TICKLESS_CFG;

    // Safety: No context switching during boot
    let tstate = unsafe { &mut *Traits::tickless_state() };

    tstate.reset(tcfg, Traits::time_lo() as u32);
}

/// Implements [`r3_kernel::PortTimer::tick_count`]
///
/// # Safety
///
/// Only meant to be referenced by `use_sbi_timer!`.
pub unsafe fn tick_count<Traits: TimerInstance>() -> UTicks {
    let tcfg = &Traits::TICKLESS_CFG;

    let hw_tick_count = Traits::time_lo() as u32;

    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *Traits::tickless_state() };
    tstate.tick_count(tcfg, hw_tick_count)
}

/// Implements [`r3_kernel::PortTimer::pend_tick`]
///
/// # Safety
///
/// Only meant to be referenced by `use_sbi_timer!`.
pub unsafe fn pend_tick<Traits: TimerInstance>() {
    Traits::set_timecmp(0);
}

/// Implements [`r3_kernel::PortTimer::pend_tick_after`]
///
/// # Safety
///
/// Only meant to be referenced by `use_sbi_timer!`.
pub unsafe fn pend_tick_after<Traits: TimerInstance>(tick_count_delta: UTicks) {
    let tcfg = &Traits::TICKLESS_CFG;
    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *Traits::tickless_state() };

    let cur_hw_tick_count = Traits::time();
    let hw_ticks = tstate
        .mark_reference_and_measure(tcfg, cur_hw_tick_count as u32, tick_count_delta)
        .hw_ticks;

    let next_hw_tick_count = cur_hw_tick_count + hw_ticks as u64;

    Traits::set_timecmp(next_hw_tick_count);
}

#[inline]
fn handle_tick<Traits: TimerInstance>(_: usize) {
    let tcfg = &Traits::TICKLESS_CFG;

    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *Traits::tickless_state() };

    let cur_hw_tick_count = Traits::time_lo() as u32;
    tstate.mark_reference(tcfg, cur_hw_tick_count);

    // Safety: CPU Lock inactive, an interrupt context
    unsafe { Traits::timer_tick() };
}
