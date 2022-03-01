//! The implementation of the `mtime`-based timer driver.
use r3_core::kernel::{traits, Cfg, StaticInterruptHandler};
use r3_kernel::{KernelTraits, PortToKernel, System, UTicks};
use r3_portkit::tickless::{TicklessCfg, TicklessOptions, TicklessStateTrait};
use tock_registers::{
    interfaces::{Readable, Writeable},
    registers::ReadWrite,
};

use crate::mtime::cfg::MtimeOptions;

/// Implemented on a kernel trait type by [`use_mtime!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_mtime!`].
pub unsafe trait TimerInstance: KernelTraits + MtimeOptions {
    const TICKLESS_CFG: TicklessCfg = match TicklessCfg::new(TicklessOptions {
        hw_freq_num: <Self as MtimeOptions>::FREQUENCY,
        hw_freq_denom: <Self as MtimeOptions>::FREQUENCY_DENOMINATOR,
        hw_headroom_ticks: <Self as MtimeOptions>::HEADROOM,
        // `mtime` is a 64-bit free-running counter and it is
        // expensive to create a 32-bit timer with an arbitrary
        // period out of it.
        force_full_hw_period: true,
        // If clearing `mtime` is not allowed, we must record the
        // starting value of `mtime` by calling `reset`.
        resettable: !<Self as MtimeOptions>::RESET_MTIME,
    }) {
        Ok(x) => x,
        Err(e) => e.panic(),
    };

    type TicklessState: TicklessStateTrait;

    fn tickless_state() -> *mut Self::TicklessState;
}

trait TimerInstanceExt: TimerInstance {
    #[inline(always)]
    fn mtime_reg32() -> &'static [ReadWrite<u32>; 2] {
        // Safety: Verified by the user of `use_mtime!`
        unsafe { &*(Self::MTIME_PTR as *const _) }
    }

    #[inline(always)]
    fn mtime_reg64() -> &'static ReadWrite<u64> {
        // Safety: Verified by the user of `use_mtime!`
        unsafe { &*(Self::MTIME_PTR as *const _) }
    }

    #[inline(always)]
    fn mtimecmp_reg32() -> &'static [ReadWrite<u32>; 2] {
        // Safety: Verified by the user of `use_mtime!`
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

    if Traits::RESET_MTIME {
        Traits::mtime_reg32()[0].set(0);
    } else {
        tstate.reset(tcfg, Traits::mtime_reg32()[0].get());
    }
}

/// Implements [`r3_kernel::PortTimer::tick_count`]
///
/// # Safety
///
/// Only meant to be referenced by `use_mtime!`.
pub unsafe fn tick_count<Traits: TimerInstance>() -> UTicks {
    let tcfg = &Traits::TICKLESS_CFG;

    let hw_tick_count = Traits::mtime_reg32()[0].get();

    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *Traits::tickless_state() };
    tstate.tick_count(tcfg, hw_tick_count)
}

/// Implements [`r3_kernel::PortTimer::pend_tick`]
///
/// # Safety
///
/// Only meant to be referenced by `use_mtime!`.
pub unsafe fn pend_tick<Traits: TimerInstance>() {
    Traits::mtimecmp_reg32()[0].set(0);
    Traits::mtimecmp_reg32()[1].set(0);
}

/// Implements [`r3_kernel::PortTimer::pend_tick_after`]
///
/// # Safety
///
/// Only meant to be referenced by `use_mtime!`.
pub unsafe fn pend_tick_after<Traits: TimerInstance>(tick_count_delta: UTicks) {
    let tcfg = &Traits::TICKLESS_CFG;
    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *Traits::tickless_state() };

    let cur_hw_tick_count = Traits::mtime();
    let hw_ticks = tstate
        .mark_reference_and_measure(tcfg, cur_hw_tick_count as u32, tick_count_delta)
        .hw_ticks;

    let next_hw_tick_count = cur_hw_tick_count + hw_ticks as u64;

    // Since we have CPU Lock, spurious timer interrupts while non-atomically
    // updating `mtimecmp` are acceptable
    Traits::mtimecmp_reg32()[0].set(next_hw_tick_count as u32);
    Traits::mtimecmp_reg32()[1].set((next_hw_tick_count >> 32) as u32);
}

#[inline]
fn handle_tick<Traits: TimerInstance>() {
    let tcfg = &Traits::TICKLESS_CFG;

    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *Traits::tickless_state() };

    let cur_hw_tick_count = Traits::mtime_reg32()[0].get();
    tstate.mark_reference(tcfg, cur_hw_tick_count);

    // Safety: CPU Lock inactive, an interrupt context
    unsafe { Traits::timer_tick() };
}
