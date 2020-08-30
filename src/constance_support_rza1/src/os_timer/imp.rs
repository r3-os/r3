//! The implementation of the RZ/A1 OS Timer driver.
use constance::kernel::{
    cfg::CfgBuilder, InterruptHandler, InterruptLine, Kernel, PortToKernel, UTicks,
};
use constance_port_arm::Gic;
use constance_portkit::tickless::{TicklessCfg, TicklessStateTrait};

use crate::os_timer::{cfg::OsTimerOptions, os_timer_regs};

/// Implemented on a system type by [`use_os_timer!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_os_timer!`].
pub unsafe trait OsTimerInstance: Kernel + OsTimerOptions + Gic {
    // FIXME: Specifying `TicklessCfg::new(...)` here causes a "cycle
    //        detected" error
    const TICKLESS_CFG: TicklessCfg;

    // FIXME: Specifying `TicklessState<{ Self::TICKLESS_CFG }>` here
    //        fails with an error message similar to
    //        <https://github.com/rust-lang/rust/issues/72821>
    type TicklessState: TicklessStateTrait;

    fn tickless_state() -> *mut Self::TicklessState;
}

trait OsTimerInstanceExt: OsTimerInstance {
    fn ostm0_regs() -> &'static os_timer_regs::OsTimer {
        // Safety: Verified by the user of `use_os_timer!`
        unsafe { &*(Self::OSTM0_BASE as *const os_timer_regs::OsTimer) }
    }
}
impl<T: OsTimerInstance> OsTimerInstanceExt for T {}

/// The configuration function.
pub const fn configure<System: OsTimerInstance>(b: &mut CfgBuilder<System>) {
    InterruptLine::build()
        .line(System::INTERRUPT_OSTM0)
        .priority(System::INTERRUPT_OSTM0_PRIORITY)
        .enabled(true)
        .finish(b);
    InterruptHandler::build()
        .line(System::INTERRUPT_OSTM0)
        .start(handle_tick::<System>)
        .finish(b);
}

/// Implements [`crate::Timer::init`]
#[inline]
pub fn init<System: OsTimerInstance>() {
    let ostm0 = System::ostm0_regs();
    let tcfg = System::TICKLESS_CFG;

    // RZ/A1x includes two instances of OS Timer. We use OSTM0 of them.
    //
    // OSTM0 will operate in Free-Running Comparison Mode, where the timer
    // counts up from `0` and generates an interrupt when the counter value
    // matches `OSTM0CMP`.
    ostm0.TT.set(1); // stop
    ostm0
        .CTL
        .write(os_timer_regs::CTL::MD0::Disable + os_timer_regs::CTL::MD1::FreeRunningComparison);
    ostm0.CMP.set(u32::MAX); // dummy - a real value will be set soon while booting
    ostm0.TS.set(1); // start

    debug_assert_eq!(tcfg.hw_max_tick_count(), u32::MAX);

    // Configure the interrupt line as edge-triggered
    System::set_interrupt_line_trigger_mode(
        System::INTERRUPT_OSTM0,
        constance_port_arm::InterruptLineTriggerMode::RisingEdge,
    )
    .unwrap();
}

fn hw_tick_count<System: OsTimerInstance>() -> u32 {
    System::ostm0_regs().CNT.get()
}

/// Implements [`constance::kernel::PortTimer::tick_count`]
///
/// # Safety
///
/// Only meant to be referenced by `use_os_timer!`.
pub unsafe fn tick_count<System: OsTimerInstance>() -> UTicks {
    let tcfg = &System::TICKLESS_CFG;

    let hw_tick_count = hw_tick_count::<System>();

    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *System::tickless_state() };
    tstate.tick_count(tcfg, hw_tick_count)
}

/// Implements [`constance::kernel::PortTimer::pend_tick`]
///
/// # Safety
///
/// Only meant to be referenced by `use_os_timer!`.
#[inline]
pub unsafe fn pend_tick<System: OsTimerInstance>() {
    let _ = InterruptLine::<System>::from_num(System::INTERRUPT_OSTM0).pend();
}

/// Implements [`constance::kernel::PortTimer::pend_tick_after`]
///
/// # Safety
///
/// Only meant to be referenced by `use_os_timer!`.
pub unsafe fn pend_tick_after<System: OsTimerInstance>(tick_count_delta: UTicks) {
    let ostm0 = System::ostm0_regs();
    let tcfg = &System::TICKLESS_CFG;
    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *System::tickless_state() };

    let cur_hw_tick_count = hw_tick_count::<System>();
    let measurement = tstate.mark_reference_and_measure(tcfg, cur_hw_tick_count, tick_count_delta);

    ostm0.CMP.set(measurement.end_hw_tick_count);

    // Did we go past `hw_tick_count` already? In that case, pend an interrupt
    // manually because the timer might not have generated an interrupt.
    let cur_hw_tick_count2 = hw_tick_count::<System>();
    if cur_hw_tick_count2.wrapping_sub(cur_hw_tick_count) >= measurement.hw_ticks {
        let _ = InterruptLine::<System>::from_num(System::INTERRUPT_OSTM0).pend();
    }
}

#[inline]
fn handle_tick<System: OsTimerInstance>(_: usize) {
    let tcfg = &System::TICKLESS_CFG;

    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *System::tickless_state() };

    let cur_hw_tick_count = hw_tick_count::<System>();
    tstate.mark_reference(tcfg, cur_hw_tick_count);

    // `timer_tick` will call `pend_tick[_after]`, so it's unnecessary to
    // clear the interrupt flag

    // Safety: CPU Lock inactive, an interrupt context
    unsafe { System::timer_tick() };
}
