//! The implementation of the SP804 Dual Timer driver.
use r3_core::kernel::{traits, Cfg, InterruptLine, StaticInterruptHandler};
use r3_kernel::{KernelTraits, PortToKernel, System, UTicks};
use r3_portkit::tickless::{TicklessCfg, TicklessOptions, TicklessStateTrait};
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

use crate::sp804::{cfg::Sp804Options, sp804_regs};

/// Implemented on a kernel trait type by [`use_sp804!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_sp804!`].
pub unsafe trait Sp804Instance: KernelTraits + Sp804Options {
    const TICKLESS_CFG: TicklessCfg = match TicklessCfg::new(TicklessOptions {
        hw_freq_num: <Self as Sp804Options>::FREQUENCY,
        hw_freq_denom: <Self as Sp804Options>::FREQUENCY_DENOMINATOR,
        hw_headroom_ticks: <Self as Sp804Options>::HEADROOM,
        force_full_hw_period: false,
        resettable: false,
    }) {
        Ok(x) => x,
        Err(e) => e.panic(),
    };

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
pub const fn configure<C, Traits: Sp804Instance>(b: &mut Cfg<C>)
where
    C: ~const traits::CfgInterruptLine<System = System<Traits>>,
{
    InterruptLine::define()
        .line(Traits::INTERRUPT_NUM)
        .priority(Traits::INTERRUPT_PRIORITY)
        .enabled(true)
        .finish(b);
    StaticInterruptHandler::define()
        .line(Traits::INTERRUPT_NUM)
        .start(handle_tick::<Traits>)
        .finish(b);
}

/// Implements [`crate::Timer::init`]
#[inline]
pub fn init<Traits: Sp804Instance>() {
    let sp804 = Traits::sp804_regs();
    let tcfg = Traits::TICKLESS_CFG;

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

fn hw_tick_count<Traits: Sp804Instance>() -> u32 {
    let sp804 = Traits::sp804_regs();
    let tcfg = Traits::TICKLESS_CFG;

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

/// Implements [`r3_kernel::PortTimer::tick_count`]
///
/// # Safety
///
/// Only meant to be referenced by `use_sp804!`.
pub unsafe fn tick_count<Traits: Sp804Instance>() -> UTicks {
    let tcfg = &Traits::TICKLESS_CFG;

    let hw_tick_count = hw_tick_count::<Traits>();

    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *Traits::tickless_state() };
    tstate.tick_count(tcfg, hw_tick_count)
}

/// Implements [`r3_kernel::PortTimer::pend_tick`]
///
/// # Safety
///
/// Only meant to be referenced by `use_sp804!`.
pub unsafe fn pend_tick<Traits: Sp804Instance>() {
    InterruptLine::<System<Traits>>::from_num(Traits::INTERRUPT_NUM)
        .pend()
        .unwrap();
}

/// Implements [`r3_kernel::PortTimer::pend_tick_after`]
///
/// # Safety
///
/// Only meant to be referenced by `use_sp804!`.
pub unsafe fn pend_tick_after<Traits: Sp804Instance>(tick_count_delta: UTicks) {
    let sp804 = Traits::sp804_regs();
    let tcfg = &Traits::TICKLESS_CFG;
    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *Traits::tickless_state() };

    let cur_hw_tick_count = hw_tick_count::<Traits>();
    let hw_ticks = tstate
        .mark_reference_and_measure(tcfg, cur_hw_tick_count, tick_count_delta)
        .hw_ticks;

    sp804
        .Timer2Control
        .modify(sp804_regs::Control::TimerEn::Disable);
    sp804.Timer2Load.set(hw_ticks);
    sp804.Timer2IntClr.set(hw_ticks); // value is irrelevant
    sp804
        .Timer2Control
        .modify(sp804_regs::Control::TimerEn::Enable);
}

#[inline]
fn handle_tick<Traits: Sp804Instance>() {
    let tcfg = &Traits::TICKLESS_CFG;

    // Safety: CPU Lock protects it from concurrent access
    let tstate = unsafe { &mut *Traits::tickless_state() };

    let cur_hw_tick_count = hw_tick_count::<Traits>();
    tstate.mark_reference(tcfg, cur_hw_tick_count);

    // `timer_tick` will call `pend_tick[_after]`, so it's unnecessary to
    // clear the interrupt flag

    // Safety: CPU Lock inactive, an interrupt context
    unsafe { Traits::timer_tick() };
}
