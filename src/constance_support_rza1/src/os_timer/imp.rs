//! The implementation of the RZ/A1 OS Timer driver.
use constance::kernel::{
    cfg::CfgBuilder, InterruptHandler, InterruptLine, Kernel, PortToKernel, UTicks,
};
use constance_port_arm::Gic;
use constance_portkit::tickless::{TicklessCfg, TicklessStateTrait};
use rza1::ostm0 as ostm;

use crate::os_timer::cfg::OsTimerOptions;

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
    fn ostm_regs() -> &'static ostm::RegisterBlock {
        // Safety: Verified by the user of `use_os_timer!`
        unsafe { &*(Self::OSTM_BASE as *const ostm::RegisterBlock) }
    }
}
impl<T: OsTimerInstance> OsTimerInstanceExt for T {}

/// The configuration function.
pub const fn configure<System: OsTimerInstance>(b: &mut CfgBuilder<System>) {
    InterruptLine::build()
        .line(System::INTERRUPT_OSTM)
        .priority(System::INTERRUPT_OSTM_PRIORITY)
        .enabled(true)
        .finish(b);
    InterruptHandler::build()
        .line(System::INTERRUPT_OSTM)
        .start(handle_tick::<System>)
        .finish(b);
}

/// Implements [`crate::Timer::init`]
#[inline]
pub fn init<System: OsTimerInstance>() {
    let ostm = System::ostm_regs();
    let tcfg = System::TICKLESS_CFG;

    // Enable clock supply
    if let Some((addr, bit)) = System::STBCR_OSTM {
        // Safety: Verified by the user of `use_os_timer!`
        unsafe {
            let ptr = addr as *mut u8;
            ptr.write_volatile(ptr.read_volatile() & !(1u8 << bit));
        }
    }

    // RZ/A1x includes two instances of OS Timer. We use one of them.
    //
    // OS Timer will operate in Free-Running Comparison Mode, where the timer
    // counts up from `0` and generates an interrupt when the counter value
    // matches `OSTMCMP`.
    ostm.tt.write(|w| w.tt().stop()); // stop
    ostm.ctl.write(|w| {
        w
            // Don't generate an interrupt on start
            .md0()
            .clear_bit()
            // Free-Running Comparison Mode
            .md1()
            .free_running_comparison()
    });
    ostm.cmp.write(|w| w.cmp().bits(u32::MAX)); // dummy - a real value will be set soon while booting
    ostm.ts.write(|w| w.ts().start()); // start

    debug_assert_eq!(tcfg.hw_max_tick_count(), u32::MAX);

    // Configure the interrupt line as edge-triggered
    System::set_interrupt_line_trigger_mode(
        System::INTERRUPT_OSTM,
        constance_port_arm::InterruptLineTriggerMode::RisingEdge,
    )
    .unwrap();
}

fn hw_tick_count<System: OsTimerInstance>() -> u32 {
    System::ostm_regs().cnt.read().bits()
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
    let _ = InterruptLine::<System>::from_num(System::INTERRUPT_OSTM).pend();
}

/// Implements [`constance::kernel::PortTimer::pend_tick_after`]
///
/// # Safety
///
/// Only meant to be referenced by `use_os_timer!`.
pub unsafe fn pend_tick_after<System: OsTimerInstance>(tick_count_delta: UTicks) {
    let ostm = System::ostm_regs();
    let tcfg = &System::TICKLESS_CFG;
    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *System::tickless_state() };

    let cur_hw_tick_count = hw_tick_count::<System>();
    let measurement = tstate.mark_reference_and_measure(tcfg, cur_hw_tick_count, tick_count_delta);

    ostm.cmp
        .write(|w| w.cmp().bits(measurement.end_hw_tick_count));

    // Did we go past `hw_tick_count` already? In that case, pend an interrupt
    // manually because the timer might not have generated an interrupt.
    let cur_hw_tick_count2 = hw_tick_count::<System>();
    if cur_hw_tick_count2.wrapping_sub(cur_hw_tick_count) >= measurement.hw_ticks {
        let _ = InterruptLine::<System>::from_num(System::INTERRUPT_OSTM).pend();
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
