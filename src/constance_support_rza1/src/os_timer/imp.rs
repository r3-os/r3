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

    fn ostm1_regs() -> &'static os_timer_regs::OsTimer {
        // Safety: Verified by the user of `use_os_timer!`
        unsafe { &*(Self::OSTM1_BASE as *const os_timer_regs::OsTimer) }
    }
}
impl<T: OsTimerInstance> OsTimerInstanceExt for T {}

/// The configuration function.
pub const fn configure<System: OsTimerInstance>(b: &mut CfgBuilder<System>) {
    InterruptLine::build()
        .line(System::INTERRUPT_OSTM1)
        .priority(System::INTERRUPT_OSTM1_PRIORITY)
        .enabled(true)
        .finish(b);
    InterruptHandler::build()
        .line(System::INTERRUPT_OSTM1)
        .start(handle_tick::<System>)
        .finish(b);
}

/// Implements [`crate::Timer::init`]
#[inline]
pub fn init<System: OsTimerInstance>() {
    let ostm0 = System::ostm0_regs();
    let ostm1 = System::ostm1_regs();
    let tcfg = System::TICKLESS_CFG;

    // RZ/A1x includes two instances of OS Timer. We use OSTM0 to track the
    // current time in real time.
    //
    // OSTM0 will operate in Interval Timer Mode, where the timer value wraps
    // around to `OSTM0CMP` after reaching `0`. The timer period is `OSTM0CMP +
    // 1` cycles.
    ostm0.TT.set(1); // stop
    ostm0
        .CTL
        .write(os_timer_regs::CTL::MD0::Disable + os_timer_regs::CTL::MD1::IntervalTimer);
    ostm0.CMP.set(tcfg.hw_max_tick_count());
    ostm0.TS.set(1); // start

    // We use OSTM1 to implement `pend_tick[_after]`.
    //
    // OSTM1 will operate in Free-Running Comparison Mode, where the timer
    // counts up from `0` and generates an interrupt when the counter value
    // matches `OSTM1CMP`.
    ostm1.TT.set(1); // stop
    ostm1
        .CTL
        .write(os_timer_regs::CTL::MD0::Disable + os_timer_regs::CTL::MD1::FreeRunningComparison);

    // Configure the interrupt line as edge-triggered
    System::set_interrupt_line_trigger_mode(
        System::INTERRUPT_OSTM1,
        constance_port_arm::InterruptLineTriggerMode::RisingEdge,
    )
    .unwrap();
}

fn hw_tick_count<System: OsTimerInstance>() -> u32 {
    let ostm0 = System::ostm0_regs();
    let tcfg = System::TICKLESS_CFG;

    let value = ostm0.CNT.get();

    let hw_tick_count = tcfg.hw_max_tick_count() - value;

    hw_tick_count
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
pub unsafe fn pend_tick<System: OsTimerInstance>() {
    InterruptLine::<System>::from_num(System::INTERRUPT_OSTM1)
        .pend()
        .unwrap();
}

/// Implements [`constance::kernel::PortTimer::pend_tick_after`]
///
/// # Safety
///
/// Only meant to be referenced by `use_os_timer!`.
pub unsafe fn pend_tick_after<System: OsTimerInstance>(tick_count_delta: UTicks) {
    let ostm1 = System::ostm1_regs();
    let tcfg = &System::TICKLESS_CFG;
    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *System::tickless_state() };

    // Clear the current timeout
    ostm1.TT.set(1); // stop
    InterruptLine::<System>::from_num(System::INTERRUPT_OSTM1)
        .clear()
        .unwrap();

    let cur_hw_tick_count = hw_tick_count::<System>();
    let hw_ticks = tstate
        .mark_reference_and_measure(tcfg, cur_hw_tick_count, tick_count_delta)
        .hw_ticks;

    // | CMP | P0ϕ Cycles Before Interrupt |
    // | --- | --------------------------- |
    // |   0 |                           2 |
    // |   1 |                           4 |
    // |   n |                       n + 3 |
    ostm1.CMP.set(hw_ticks.saturating_sub(4) + 1);
    ostm1.TS.set(1); // start
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
