//! The tickful `PortTimer` implementation based on SysTick.
use core::cell::UnsafeCell;
use r3::{
    kernel::{raw, traits, Cfg, InterruptLine, StartupHook, StaticInterruptHandler},
    utils::ConstDefault,
};
use r3_kernel::{KernelTraits, PortToKernel, System, UTicks};
use r3_portkit::tickful::{TickfulCfg, TickfulOptions, TickfulState, TickfulStateTrait};

use crate::{SysTickOptions, INTERRUPT_SYSTICK};

/// Implemented on a system type by [`use_systick_tickful!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_systick_tickful!`].
pub unsafe trait SysTickTickfulInstance: KernelTraits + SysTickOptions {
    const TICKFUL_CFG: TickfulCfg = if Self::TICK_PERIOD > 0x100_0000 {
        panic!("the tick period measured in cycles must be in range `0..=0x1000000`");
    } else {
        match TickfulCfg::new(TickfulOptions {
            hw_freq_num: Self::FREQUENCY,
            hw_freq_denom: Self::FREQUENCY_DENOMINATOR,
            hw_tick_period: Self::TICK_PERIOD,
        }) {
            Ok(x) => x,
            Err(e) => e.panic(),
        }
    };

    /// Handle a SysTick interrupt.
    ///
    /// # Safety
    ///
    /// Interrupt context, CPU Lock inactive
    unsafe fn handle_tick();
}

/// The configuration function.
pub const fn configure<C, Traits: SysTickTickfulInstance>(b: &mut Cfg<C>)
where
    C: ~const traits::CfgBase<System = System<Traits>> + ~const traits::CfgInterruptLine,
{
    InterruptLine::define()
        .line(INTERRUPT_SYSTICK)
        .priority(Traits::INTERRUPT_PRIORITY)
        .finish(b);
    StaticInterruptHandler::define()
        .line(INTERRUPT_SYSTICK)
        .start(
            #[inline]
            |_| unsafe { Traits::handle_tick() },
        )
        .finish(b);

    StartupHook::define()
        .start(
            #[inline]
            |_| init(Traits::TICK_PERIOD),
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
pub type State<Traits: SysTickTickfulInstance> =
    StateCore<TickfulState<{ <Traits as SysTickTickfulInstance>::TICKFUL_CFG }>>;

pub struct StateCore<TickfulState> {
    inner: UnsafeCell<TickfulState>,
}

// Safety: `inner` is protected from concurrent access by CPU Lock
unsafe impl<TickfulState> Sync for StateCore<TickfulState> {}

impl<TickfulState: ConstDefault> ConstDefault for StateCore<TickfulState> {
    const DEFAULT: Self = Self {
        inner: ConstDefault::DEFAULT,
    };
}

impl<TickfulState: TickfulStateTrait> StateCore<TickfulState> {
    /// Handle a SysTick interrupt.
    ///
    /// # Safety
    ///
    /// Interrupt context, CPU Lock inactive
    #[inline]
    pub unsafe fn handle_tick<Traits: SysTickTickfulInstance>(&self) {
        <System<Traits> as raw::KernelBase>::raw_acquire_cpu_lock().unwrap();

        // Safety: CPU Lock protects it from concurrent access
        let inner = unsafe { &mut *self.inner.get() };

        inner.tick(&Traits::TICKFUL_CFG);

        // Safety: We own the CPU Lock, we are not in a boot context
        unsafe { <System<Traits> as raw::KernelBase>::raw_release_cpu_lock().unwrap() };

        // Safety: CPU Lock inactive, an interrupt context
        unsafe { Traits::timer_tick() };
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
