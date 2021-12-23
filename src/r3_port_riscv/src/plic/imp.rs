/// The implementation of the Platform-Level Interrupt Controller driver.
use r3_core::kernel::{
    traits, Cfg, EnableInterruptLineError, InterruptNum, InterruptPriority,
    QueryInterruptLineError, SetInterruptLinePriorityError, StaticInterruptHandler,
};
use r3_kernel::{KernelTraits, System};
use tock_registers::interfaces::{Readable, Writeable};

use crate::{Plic, INTERRUPT_EXTERNAL, INTERRUPT_PLATFORM_START};

/// The configuration function.
pub const fn configure<C, Traits: Plic + KernelTraits>(b: &mut Cfg<C>)
where
    C: ~const traits::CfgInterruptLine<System = System<Traits>>,
{
    StaticInterruptHandler::define()
        .line(INTERRUPT_EXTERNAL)
        .start(interrupt_handler::<Traits>)
        .finish(b);
}

/// Implements [`crate::InterruptController::init`].
pub fn init<Traits: Plic>() {
    let plic_regs = Traits::plic_regs();
    let ctx = Traits::CONTEXT;
    let num_ints = Traits::MAX_NUM + 1;

    // Disable all interrupts
    for i in 0..(num_ints + 31) / 32 {
        plic_regs.interrupt_enable[ctx][i].set(0);
    }

    // Change the priority thread of the current context
    // to accept all interrupts
    plic_regs.ctxs[Traits::CONTEXT].priority_threshold.set(0);
}

#[inline]
fn interrupt_handler<Traits: Plic + KernelTraits>(_: usize) {
    if let Some((token, num)) = claim_interrupt::<Traits>() {
        if let Some(handler) = Traits::INTERRUPT_HANDLERS.get(num) {
            if Traits::USE_NESTING {
                unsafe { riscv::register::mie::set_mext() };
            }

            // Safety: The interrupt controller driver is responsible for
            //         dispatching the appropriate interrupt handler for
            //         a platform interrupt
            unsafe { handler() };

            if Traits::USE_NESTING {
                unsafe { riscv::register::mie::clear_mext() };
            }
        }

        end_interrupt::<Traits>(token);
    }
}

type Token = u32;

#[inline]
fn claim_interrupt<Traits: Plic>() -> Option<(Token, InterruptNum)> {
    let plic_regs = Traits::plic_regs();

    let num = plic_regs.ctxs[Traits::CONTEXT].claim_complete.get();
    if num == 0 {
        return None;
    }
    if Traits::USE_NESTING {
        // Raise the priority threshold to mask the claimed interrupt
        let old_threshold = plic_regs.ctxs[Traits::CONTEXT].priority_threshold.get();
        let priority = plic_regs.interrupt_priority[num as usize].get();

        plic_regs.ctxs[Traits::CONTEXT]
            .priority_threshold
            .set(priority);

        // Allow other interrupts to be taken by completing this one
        plic_regs.ctxs[Traits::CONTEXT].claim_complete.set(num);

        Some((
            old_threshold,
            num as InterruptNum + INTERRUPT_PLATFORM_START,
        ))
    } else {
        Some((num, num as InterruptNum + INTERRUPT_PLATFORM_START))
    }
}

#[inline]
fn end_interrupt<Traits: Plic>(token: Token) {
    let plic_regs = Traits::plic_regs();

    if Traits::USE_NESTING {
        plic_regs.ctxs[Traits::CONTEXT]
            .priority_threshold
            .set(token);
    } else {
        plic_regs.ctxs[Traits::CONTEXT].claim_complete.set(token);
    }
}

/// Implements [`crate::InterruptController::set_interrupt_line_priority`].
pub fn set_interrupt_line_priority<Traits: Plic>(
    line: InterruptNum,
    priority: InterruptPriority,
) -> Result<(), SetInterruptLinePriorityError> {
    let plic_regs = Traits::plic_regs();
    let line = line - INTERRUPT_PLATFORM_START;

    if line > Traits::MAX_NUM || priority < 0 || priority > Traits::MAX_PRIORITY {
        return Err(SetInterruptLinePriorityError::BadParam);
    }

    plic_regs.interrupt_priority[line].set(priority as u32);
    Ok(())
}

/// Implements [`crate::InterruptController::enable_interrupt_line`].
pub fn enable_interrupt_line<Traits: Plic>(
    line: InterruptNum,
) -> Result<(), EnableInterruptLineError> {
    let plic_regs = Traits::plic_regs();
    let line = line - INTERRUPT_PLATFORM_START;

    if line > Traits::MAX_NUM {
        return Err(EnableInterruptLineError::BadParam);
    }

    let reg = &plic_regs.interrupt_enable[Traits::CONTEXT][line / 32];
    reg.set(reg.get() | (1u32 << (line % 32)));

    Ok(())
}

/// Implements [`crate::InterruptController::disable_interrupt_line`].
pub fn disable_interrupt_line<Traits: Plic>(
    line: InterruptNum,
) -> Result<(), EnableInterruptLineError> {
    let plic_regs = Traits::plic_regs();
    let line = line - INTERRUPT_PLATFORM_START;

    if line > Traits::MAX_NUM {
        return Err(EnableInterruptLineError::BadParam);
    }

    let reg = &plic_regs.interrupt_enable[Traits::CONTEXT][line / 32];
    reg.set(reg.get() & !(1u32 << (line % 32)));

    Ok(())
}

/// Implements [`crate::InterruptController::is_interrupt_line_pending`].
pub fn is_interrupt_line_pending<Traits: Plic>(
    line: InterruptNum,
) -> Result<bool, QueryInterruptLineError> {
    let plic_regs = Traits::plic_regs();
    let line = line - INTERRUPT_PLATFORM_START;

    if line > Traits::MAX_NUM {
        return Err(QueryInterruptLineError::BadParam);
    }

    Ok((plic_regs.interrupt_pending[line / 32].get() & (1u32 << (line % 32))) != 0)
}
