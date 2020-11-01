//! The interrupt controller driver based on [`r3_port_riscv::use_plic!`]
//! that simulates pend/clear operations (which are not supported by PLIC) by
//! toggling GPIO ports.
//!
//! GPIO pins 0 and 1 must not be driven externally.
use r3::kernel::{
    cfg::CfgBuilder, ClearInterruptLineError, InterruptHandler, InterruptNum, Kernel,
    PendInterruptLineError,
};
use core::sync::atomic::Ordering;

#[macro_export]
macro_rules! use_interrupt_e310x {
    (unsafe impl InterruptController for $sys:ty) => {
        const _: () = {
            use r3::kernel::{
                cfg::CfgBuilder, ClearInterruptLineError, EnableInterruptLineError, InterruptNum,
                InterruptPriority, PendInterruptLineError, QueryInterruptLineError,
                SetInterruptLinePriorityError,
            };
            use r3_port_riscv::{
                plic::{imp, plic_regs},
                InterruptController, Plic, PlicOptions,
            };
            use core::ops::Range;

            unsafe impl Plic for $sys {
                fn plic_regs() -> &'static plic_regs::Plic {
                    unsafe { &*(<$sys as PlicOptions>::PLIC_BASE as *const plic_regs::Plic) }
                }
            }

            impl $sys {
                pub const fn configure_interrupt(b: &mut CfgBuilder<Self>) {
                    imp::configure::<Self>(b);
                    crate::interrupt_e310x::configure(b);
                }
            }

            impl PlicOptions for System {
                const MAX_PRIORITY: InterruptPriority = 7;
                const MAX_NUM: InterruptNum = 127;
                const PLIC_BASE: usize = 0x0c00_0000;
                // The nesting trick can't be used on a real FE310 because
                // its PLIC doesn't clear the pending flag when an incoming
                // interrupt request signal is deasserted.
                #[cfg(feature = "board-e310x-qemu")]
                const USE_NESTING: bool = true;
            }

            impl InterruptController for $sys {
                #[inline]
                unsafe fn init() {
                    imp::init::<Self>();
                    crate::interrupt_e310x::init();
                }

                const MANAGED_INTERRUPT_PRIORITY_RANGE: Range<InterruptPriority> =
                    0..(<$sys as PlicOptions>::MAX_PRIORITY + 1);

                #[inline]
                unsafe fn set_interrupt_line_priority(
                    line: InterruptNum,
                    priority: InterruptPriority,
                ) -> Result<(), SetInterruptLinePriorityError> {
                    imp::set_interrupt_line_priority::<Self>(line, priority)
                }

                #[inline]
                unsafe fn enable_interrupt_line(
                    line: InterruptNum,
                ) -> Result<(), EnableInterruptLineError> {
                    imp::enable_interrupt_line::<Self>(line)
                }

                #[inline]
                unsafe fn disable_interrupt_line(
                    line: InterruptNum,
                ) -> Result<(), EnableInterruptLineError> {
                    imp::disable_interrupt_line::<Self>(line)
                }

                #[inline]
                unsafe fn pend_interrupt_line(
                    line: InterruptNum,
                ) -> Result<(), PendInterruptLineError> {
                    crate::interrupt_e310x::pend_interrupt_line(line)
                }

                #[inline]
                unsafe fn clear_interrupt_line(
                    line: InterruptNum,
                ) -> Result<(), ClearInterruptLineError> {
                    if cfg!(feature = "board-e310x-qemu") {
                        crate::interrupt_e310x::clear_interrupt_line(line)
                    } else {
                        Err(ClearInterruptLineError::NotSupported)
                    }
                }

                #[inline]
                unsafe fn is_interrupt_line_pending(
                    line: InterruptNum,
                ) -> Result<bool, QueryInterruptLineError> {
                    imp::is_interrupt_line_pending::<Self>(line)
                }
            }
        };
    };
}

#[allow(dead_code)]
mod gpio0 {
    use core::{mem::transmute, sync::atomic::AtomicU32};
    use e310x::GPIO0;

    macro gen($($field:ident,)*) {
        $(
            pub fn $field() -> &'static AtomicU32 {
                unsafe {
                    let gpio_regs = &*GPIO0::ptr();
                    transmute(&gpio_regs.$field)
                }
            }
        )*
    }

    gen!(
        input_val, input_en, output_en, output_val, pullup, drive, rise_ie, rise_ip, fall_ie,
        fall_ip, high_ie, high_ip, low_ie, low_ip, iof_en, iof_sel, out_xor,
    );
}

#[inline]
pub(crate) fn init() {
    let port_mask = 0b11; // pin0 and pin1

    // Configure the pins for output
    gpio0::drive().fetch_and(!port_mask, Ordering::Relaxed);
    gpio0::out_xor().fetch_and(!port_mask, Ordering::Relaxed);
    gpio0::output_en().fetch_or(port_mask, Ordering::Relaxed);
    gpio0::iof_en().fetch_and(!port_mask, Ordering::Relaxed);
    gpio0::output_val().fetch_and(!port_mask, Ordering::Relaxed);

    // Generate an interrupt when these pins are high
    gpio0::input_en().fetch_or(port_mask, Ordering::Relaxed);
    gpio0::rise_ie().fetch_and(!port_mask, Ordering::Relaxed);
    gpio0::fall_ie().fetch_and(!port_mask, Ordering::Relaxed);
    gpio0::high_ie().fetch_or(port_mask, Ordering::Relaxed);
    gpio0::low_ie().fetch_and(!port_mask, Ordering::Relaxed);
    gpio0::high_ip().store(port_mask, Ordering::Relaxed);
}

pub(crate) const INTERRUPT_GPIO0: InterruptNum =
    r3_port_riscv::INTERRUPT_PLATFORM_START + e310x::Interrupt::GPIO0 as InterruptNum;
pub(crate) const INTERRUPT_GPIO1: InterruptNum =
    r3_port_riscv::INTERRUPT_PLATFORM_START + e310x::Interrupt::GPIO1 as InterruptNum;

/// The configuration function.
pub(crate) const fn configure<System: Kernel>(b: &mut CfgBuilder<System>) -> () {
    // Automatically clear the interrupt line when an interrupt is taken
    unsafe {
        InterruptHandler::build()
            .line(INTERRUPT_GPIO0)
            .start(|_| clear_interrupt_line(INTERRUPT_GPIO0).unwrap())
            .priority(i32::MIN)
            .unmanaged()
            .finish(b);

        InterruptHandler::build()
            .line(INTERRUPT_GPIO1)
            .start(|_| clear_interrupt_line(INTERRUPT_GPIO1).unwrap())
            .priority(i32::MIN)
            .unmanaged()
            .finish(b);
    }
}

#[inline]
pub(crate) fn pend_interrupt_line(line: InterruptNum) -> Result<(), PendInterruptLineError> {
    let pin = match line {
        INTERRUPT_GPIO0 => 0,
        INTERRUPT_GPIO1 => 1,
        _ => return Err(PendInterruptLineError::BadParam),
    };

    // Output `1`
    gpio0::output_val().fetch_or(1 << pin, Ordering::Relaxed);

    Ok(())
}

#[inline]
pub(crate) fn clear_interrupt_line(line: InterruptNum) -> Result<(), ClearInterruptLineError> {
    let pin = match line {
        INTERRUPT_GPIO0 => 0,
        INTERRUPT_GPIO1 => 1,
        _ => return Err(ClearInterruptLineError::BadParam),
    };

    // Output `0`
    gpio0::output_val().fetch_and(!(1u32 << pin), Ordering::Relaxed);

    // Clear the interrupt pending flag of the pin
    gpio0::high_ip().store(1 << pin, Ordering::Relaxed);

    Ok(())
}
