//! The implementation of the SP804 Dual Timer driver.
use constance::kernel::{
    cfg::CfgBuilder, InterruptHandler, InterruptLine, Kernel, PortToKernel, UTicks,
};
use constance_portkit::tickless::{TicklessCfg, TicklessStateTrait};

use crate::sp804::{cfg::Sp804Options, sp804_regs};

/// Implemented on a system type by [`use_sp804!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_sp804!`].
pub unsafe trait Sp804Instance: Kernel + Sp804Options {
    // FIXME: Specifying `TicklessCfg::new(...)` here causes a "cycle
    //        detected" error
    const TICKLESS_CFG: TicklessCfg;

    // FIXME: Specifying `TicklessState<{ Self::TICKLESS_CFG }>` here
    //        fails with an error message similar to
    //        <https://github.com/rust-lang/rust/issues/72821>
    type TicklessState: TicklessStateTrait;

    fn tickless_state() -> *mut Self::TicklessState;
}

trait Sp804InstanceExt: Sp804Instance {
    fn sp804_regs() -> &'static sp804_regs::Sp804 {
        // Safety: Verified by the user of `use_sp804!`
        unsafe { &*(Self::SP804_BASE as *const sp804_regs::Sp804) }
    }
}
impl<T: Sp804Instance> Sp804InstanceExt for T {}

/// The configuration function.
pub const fn configure<System: Sp804Instance>(b: &mut CfgBuilder<System>) {
    InterruptLine::build()
        .line(System::INTERRUPT_NUM)
        .priority(System::INTERRUPT_PRIORITY)
        .enabled(true)
        .finish(b);
    InterruptHandler::build()
        .line(System::INTERRUPT_NUM)
        .start(handle_tick::<System>)
        .finish(b);
}

/// Implements [`crate::Timer::init`]
#[inline]
pub fn init<System: Sp804Instance>() {
    let sp804 = System::sp804_regs();
    let tcfg = System::TICKLESS_CFG;

    // Each dual timer unit includes two instances of timer. We use Timer1 to
    // track the current time in real time.
    //
    // If `hw_max_tick_count == u32::MAX`, the timer will operate in Free-
    // running mode, where the timer value wraps around to `0xffffffff` after
    // reaching zero. The timer period in this case is 2³² cycles.
    //
    // If `hw_max_tick_count < u32::MAX`, the timer will operate in Periodic
    // mode, where the timer value is reloaded with `hw_max_tick_count + 1`
    // upon reaching zero. The timer period in this case is `hw_max_tick_count
    // + 1` cycles.
    let full_period = tcfg.hw_max_tick_count() == u32::MAX;
    sp804.Timer1Control.write(
        sp804_regs::Control::OneShot::Wrapping
            + sp804_regs::Control::TimerSize::ThirtyTwoBits
            + sp804_regs::Control::TimerPre::DivideBy1
            + sp804_regs::Control::IntEnable::Disable
            + if full_period {
                sp804_regs::Control::TimerMode::FreeRunning
            } else {
                sp804_regs::Control::TimerMode::Periodic
            }
            + sp804_regs::Control::TimerEn::Disable,
    );
    sp804.Timer1Load.set(if full_period {
        0xffffffff
    } else {
        tcfg.hw_max_tick_count() + 1
    });
    sp804
        .Timer1Control
        .modify(sp804_regs::Control::TimerEn::Enable);

    // We use Timer2 to implement `pend_tick[_after]`.
    //
    // The kernel will call `pend_tick_after` before releasing CPU Lock, so just
    // load a dummy counter valeu here.
    sp804.Timer2Control.write(
        sp804_regs::Control::TimerSize::ThirtyTwoBits + sp804_regs::Control::TimerEn::Disable,
    );
    sp804.Timer2Load.set(0xffffffff);
    sp804.Timer2Control.write(
        sp804_regs::Control::OneShot::OneShot
            + sp804_regs::Control::TimerSize::ThirtyTwoBits
            + sp804_regs::Control::TimerPre::DivideBy1
            + sp804_regs::Control::IntEnable::Enable
            + sp804_regs::Control::TimerEn::Enable,
    );
}

fn hw_tick_count<System: Sp804Instance>() -> u32 {
    let sp804 = System::sp804_regs();
    let tcfg = System::TICKLESS_CFG;

    let value = sp804.Timer1Value.get();

    let full_period = tcfg.hw_max_tick_count() == u32::MAX;
    let hw_tick_count = if full_period {
        !value
    } else {
        tcfg.hw_max_tick_count() + 1 - value
    };
    debug_assert!(hw_tick_count <= tcfg.hw_max_tick_count());

    hw_tick_count
}

/// Implements [`constance::kernel::PortTimer::tick_count`]
///
/// # Safety
///
/// Only meant to be referenced by `use_sp804!`.
pub unsafe fn tick_count<System: Sp804Instance>() -> UTicks {
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
/// Only meant to be referenced by `use_sp804!`.
pub unsafe fn pend_tick<System: Sp804Instance>() {
    InterruptLine::<System>::from_num(System::INTERRUPT_NUM)
        .pend()
        .unwrap();
}

/// Implements [`constance::kernel::PortTimer::pend_tick_after`]
///
/// # Safety
///
/// Only meant to be referenced by `use_sp804!`.
pub unsafe fn pend_tick_after<System: Sp804Instance>(tick_count_delta: UTicks) {
    let sp804 = System::sp804_regs();
    let tcfg = &System::TICKLESS_CFG;
    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *System::tickless_state() };

    let cur_hw_tick_count = hw_tick_count::<System>();
    let cur_tick_count = tstate.mark_reference(tcfg, cur_hw_tick_count);
    let next_tick_count = add_mod(cur_tick_count, tick_count_delta, tcfg.max_tick_count());
    let next_hw_tick_count = tstate.tick_count_to_hw_tick_count(tcfg, next_tick_count);
    let len_hw_tick_count = sub_mod(
        next_hw_tick_count,
        cur_hw_tick_count,
        tcfg.hw_max_tick_count(),
    );

    sp804
        .Timer2Control
        .modify(sp804_regs::Control::TimerEn::Disable);
    sp804.Timer2Load.set(len_hw_tick_count);
    sp804.Timer2IntClr.set(len_hw_tick_count); // value is irrelevant
    sp804
        .Timer2Control
        .modify(sp804_regs::Control::TimerEn::Enable);
}

#[inline]
fn handle_tick<System: Sp804Instance>(_: usize) {
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

#[track_caller]
#[inline]
fn add_mod(x: u32, y: u32, max: u32) -> u32 {
    debug_assert!(x <= max);
    debug_assert!(y <= max);
    if max == u32::MAX || (max - x) >= y {
        x.wrapping_add(y)
    } else {
        x.wrapping_add(y).wrapping_add(u32::MAX - max)
    }
}

#[track_caller]
#[inline]
fn sub_mod(x: u32, y: u32, max: u32) -> u32 {
    debug_assert!(x <= max);
    debug_assert!(y <= max);
    if max == u32::MAX || y < x {
        x.wrapping_sub(y)
    } else {
        x.wrapping_sub(y).wrapping_sub(u32::MAX - max)
    }
}
