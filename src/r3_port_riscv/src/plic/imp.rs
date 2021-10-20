/// The implementation of the Platform-Level Interrupt Controller driver.
use r3::kernel::{
    cfg::CfgBuilder, EnableInterruptLineError, InterruptHandler, InterruptNum, InterruptPriority,
    Kernel, QueryInterruptLineError, SetInterruptLinePriorityError,
};
use tock_registers::interfaces::{Readable, Writeable};

use crate::{Plic, INTERRUPT_EXTERNAL, INTERRUPT_PLATFORM_START};

/// The configuration function.
pub const fn configure<System: Plic + Kernel>(b: &mut CfgBuilder<System>) {
    InterruptHandler::build()
        .line(INTERRUPT_EXTERNAL)
        .start(interrupt_handler::<System>)
        .finish(b);
}

/// Implements [`crate::InterruptController::init`].
pub fn init<System: Plic>() {
    let plic_regs = System::plic_regs();
    let ctx = System::CONTEXT;
    let num_ints = System::MAX_NUM + 1;

    // Disable all interrupts
    for i in 0..(num_ints + 31) / 32 {
        plic_regs.interrupt_enable[ctx][i].set(0);
    }

    // Change the priority thread of the current context
    // to accept all interrupts
    plic_regs.ctxs[System::CONTEXT].priority_threshold.set(0);
}

#[inline]
fn interrupt_handler<System: Plic + Kernel>(_: usize) {
    if let Some((token, num)) = claim_interrupt::<System>() {
        if let Some(handler) = System::INTERRUPT_HANDLERS.get(num) {
            if System::USE_NESTING {
                unsafe { riscv::register::mie::set_mext() };
            }

            // Safety: The interrupt controller driver is responsible for
            //         dispatching the appropriate interrupt handler for
            //         a platform interrupt
            unsafe { handler() };

            if System::USE_NESTING {
                unsafe { riscv::register::mie::clear_mext() };
            }
        }

        end_interrupt::<System>(token);
    }
}

type Token = u32;

#[inline]
fn claim_interrupt<System: Plic>() -> Option<(Token, InterruptNum)> {
    let plic_regs = System::plic_regs();

    let num = plic_regs.ctxs[System::CONTEXT].claim_complete.get();
    if num == 0 {
        return None;
    }
    if System::USE_NESTING {
        // Raise the priority threshold to mask the claimed interrupt
        let old_threshold = plic_regs.ctxs[System::CONTEXT].priority_threshold.get();
        let priority = plic_regs.interrupt_priority[num as usize].get();

        plic_regs.ctxs[System::CONTEXT]
            .priority_threshold
            .set(priority);

        // Allow other interrupts to be taken by completing this one
        plic_regs.ctxs[System::CONTEXT].claim_complete.set(num);

        Some((
            old_threshold,
            num as InterruptNum + INTERRUPT_PLATFORM_START,
        ))
    } else {
        Some((num, num as InterruptNum + INTERRUPT_PLATFORM_START))
    }
}

#[inline]
fn end_interrupt<System: Plic>(token: Token) {
    let plic_regs = System::plic_regs();

    if System::USE_NESTING {
        plic_regs.ctxs[System::CONTEXT]
            .priority_threshold
            .set(token);
    } else {
        plic_regs.ctxs[System::CONTEXT].claim_complete.set(token);
    }
}

/// Implements [`crate::InterruptController::set_interrupt_line_priority`].
pub fn set_interrupt_line_priority<System: Plic>(
    line: InterruptNum,
    priority: InterruptPriority,
) -> Result<(), SetInterruptLinePriorityError> {
    let plic_regs = System::plic_regs();
    let line = line - INTERRUPT_PLATFORM_START;

    if line > System::MAX_NUM || priority < 0 || priority > System::MAX_PRIORITY {
        return Err(SetInterruptLinePriorityError::BadParam);
    }

    plic_regs.interrupt_priority[line].set(priority as u32);
    Ok(())
}

/// Implements [`crate::InterruptController::enable_interrupt_line`].
pub fn enable_interrupt_line<System: Plic>(
    line: InterruptNum,
) -> Result<(), EnableInterruptLineError> {
    let plic_regs = System::plic_regs();
    let line = line - INTERRUPT_PLATFORM_START;

    if line > System::MAX_NUM {
        return Err(EnableInterruptLineError::BadParam);
    }

    let reg = &plic_regs.interrupt_enable[System::CONTEXT][line / 32];
    reg.set(reg.get() | (1u32 << (line % 32)));

    Ok(())
}

/// Implements [`crate::InterruptController::disable_interrupt_line`].
pub fn disable_interrupt_line<System: Plic>(
    line: InterruptNum,
) -> Result<(), EnableInterruptLineError> {
    let plic_regs = System::plic_regs();
    let line = line - INTERRUPT_PLATFORM_START;

    if line > System::MAX_NUM {
        return Err(EnableInterruptLineError::BadParam);
    }

    let reg = &plic_regs.interrupt_enable[System::CONTEXT][line / 32];
    reg.set(reg.get() & !(1u32 << (line % 32)));

    Ok(())
}

/// Implements [`crate::InterruptController::is_interrupt_line_pending`].
pub fn is_interrupt_line_pending<System: Plic>(
    line: InterruptNum,
) -> Result<bool, QueryInterruptLineError> {
    let plic_regs = System::plic_regs();
    let line = line - INTERRUPT_PLATFORM_START;

    if line > System::MAX_NUM {
        return Err(QueryInterruptLineError::BadParam);
    }

    Ok((plic_regs.interrupt_pending[line / 32].get() & (1u32 << (line % 32))) != 0)
}
