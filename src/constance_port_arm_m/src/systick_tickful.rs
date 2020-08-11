//! The tickful `PortTimer` implementation based on SysTick.
use constance::{
    kernel::{
        cfg::CfgBuilder, InterruptHandler, InterruptLine, Kernel, PortToKernel, StartupHook, UTicks,
    },
    utils::Init,
};
use constance_portkit::tickful::{TickfulCfg, TickfulOptions, TickfulState, TickfulStateTrait};
use core::cell::UnsafeCell;

use super::{SysTickOptions, INTERRUPT_SYSTICK};

/// Implemented on a system type by [`use_systick_tickful!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_systick_tickful!`].
pub unsafe trait SysTickTickfulInstance: Kernel + SysTickOptions {
    const TICKFUL_CFG: TickfulCfg = match TickfulCfg::new(TickfulOptions {
        hw_freq_num: Self::FREQUENCY,
        hw_freq_denom: Self::FREQUENCY_DENOMINATOR,
        hw_tick_period: Self::TICK_PERIOD as u64,
    }) {
        Ok(x) => x,
        Err(e) => e.panic(),
    };

    /// Handle a SysTick interrupt.
    ///
    /// # Safety
    ///
    /// Interrupt context, CPU Lock inactive
    unsafe fn handle_tick();
}

/// The configuration function.
pub const fn configure<System: SysTickTickfulInstance>(b: &mut CfgBuilder<System>) -> () {
    InterruptLine::build()
        .line(INTERRUPT_SYSTICK)
        .priority(System::INTERRUPT_PRIORITY)
        .finish(b);
    InterruptHandler::build()
        .line(INTERRUPT_SYSTICK)
        .start(
            #[inline]
            |_| unsafe { System::handle_tick() },
        )
        .finish(b);

    StartupHook::build()
        .start(
            #[inline]
            |_| init(System::TICK_PERIOD),
        )
        .finish(b);
}

/// Configure SysTick.
#[inline]
fn init(period: u32) {
    // Safety: We have the control of SysTick
    let mut peripherals = unsafe { cortex_m::Peripherals::steal() };
    peripherals.SYST.set_reload(period - 1);
    peripherals.SYST.clear_current();
    peripherals.SYST.enable_interrupt();
    peripherals.SYST.enable_counter();
}

// FIXME: “bounds on generic parameters are not enforced in type aliases”
//        But it's actually required for this to type-check
#[allow(type_alias_bounds)]
pub type State<System: SysTickTickfulInstance> =
    StateCore<TickfulState<{ <System as SysTickTickfulInstance>::TICKFUL_CFG }>>;

pub struct StateCore<TickfulState> {
    inner: UnsafeCell<TickfulState>,
}

// Safety: `inner` is protected from concurrent access by CPU Lock
unsafe impl<TickfulState> Sync for StateCore<TickfulState> {}

impl<TickfulState: Init> Init for StateCore<TickfulState> {
    const INIT: Self = Self { inner: Init::INIT };
}

impl<TickfulState: TickfulStateTrait> StateCore<TickfulState> {
    /// Handle a SysTick interrupt.
    ///
    /// # Safety
    ///
    /// Interrupt context, CPU Lock inactive
    #[inline]
    pub unsafe fn handle_tick<System: SysTickTickfulInstance>(&self) {
        System::acquire_cpu_lock().unwrap();

        // Safety: CPU Lock protects it from concurrent access
        let inner = unsafe { &mut *self.inner.get() };

        inner.tick(&System::TICKFUL_CFG);

        // Safety: We own the CPU Lock, we are not in a boot context
        unsafe { System::release_cpu_lock().unwrap() };

        // Safety: CPU Lock inactive, an interrupt context
        unsafe { System::timer_tick() };
    }

    /// Implements `PortTimer::tick_count`.
    ///
    /// # Safety
    ///
    /// CPU Lock active
    pub unsafe fn tick_count(&self) -> UTicks {
        // Safety: CPU Lock protects it from concurrent access
        let inner = unsafe { &mut *self.inner.get() };

        inner.tick_count()
    }
}
