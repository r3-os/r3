/// The implementation of the Platform-Level Interrupt Controller driver.
use constance::kernel::{
    EnableInterruptLineError, InterruptNum, InterruptPriority, QueryInterruptLineError,
    SetInterruptLinePriorityError,
};

use crate::Plic;

/// Implements [`crate::InterruptController::init`].
pub fn init<System: Plic>() {
    let plic_regs = System::plic_regs();
    let ctx = System::CONTEXT;
    let num_ints = System::MAX_NUM + 1;

    // Disable all interrupts
    for i in 0..(num_ints + 31) / 32 {
        plic_regs.interrupt_enable[ctx][i].set(0);
    }
}

pub type Token = u32;

/// Implements [`crate::InterruptController::claim_interrupt`].
#[inline]
pub fn claim_interrupt<System: Plic>() -> Option<(Token, InterruptNum)> {
    let plic_regs = System::plic_regs();

    let num = plic_regs.ctxs[System::CONTEXT].claim_complete.get();
    if num == 0 {
        None
    } else {
        // Raise the priority threshold to mask the claimed interrupt
        let old_threshold = plic_regs.ctxs[System::CONTEXT].priority_threshold.get();
        let priority = plic_regs.interrupt_priority[num as usize].get();

        plic_regs.ctxs[System::CONTEXT]
            .priority_threshold
            .set(priority);

        // Allow other interrupts to be taken by completing this one
        plic_regs.ctxs[System::CONTEXT].claim_complete.set(num);

        Some((old_threshold, num as InterruptNum))
    }
}

/// Implements [`crate::InterruptController::end_interrupt`].
#[inline]
pub fn end_interrupt<System: Plic>(token: Token) {
    let plic_regs = System::plic_regs();

    plic_regs.ctxs[System::CONTEXT]
        .priority_threshold
        .set(token);
}

/// Implements [`constance::kernel::PortInterrupts::set_interrupt_line_priority`].
pub fn set_interrupt_line_priority<System: Plic>(
    line: InterruptNum,
    priority: InterruptPriority,
) -> Result<(), SetInterruptLinePriorityError> {
    let plic_regs = System::plic_regs();

    if line > System::MAX_NUM || priority < 0 || priority > System::MAX_PRIORITY {
        return Err(SetInterruptLinePriorityError::BadParam);
    }

    plic_regs.interrupt_priority[line].set(priority as u32);
    Ok(())
}

/// Implements [`constance::kernel::PortInterrupts::enable_interrupt_line`].
pub fn enable_interrupt_line<System: Plic>(
    line: InterruptNum,
) -> Result<(), EnableInterruptLineError> {
    let plic_regs = System::plic_regs();

    if line > System::MAX_NUM {
        return Err(EnableInterruptLineError::BadParam);
    }

    let reg = &plic_regs.interrupt_enable[System::CONTEXT][line / 32];
    reg.set(reg.get() | (1u32 << (line % 32)));

    Ok(())
}

/// Implements [`constance::kernel::PortInterrupts::disable_interrupt_line`].
pub fn disable_interrupt_line<System: Plic>(
    line: InterruptNum,
) -> Result<(), EnableInterruptLineError> {
    let plic_regs = System::plic_regs();

    if line > System::MAX_NUM {
        return Err(EnableInterruptLineError::BadParam);
    }

    let reg = &plic_regs.interrupt_enable[System::CONTEXT][line / 32];
    reg.set(reg.get() & !(1u32 << (line % 32)));

    Ok(())
}

/// Implements [`constance::kernel::PortInterrupts::is_interrupt_line_pending`].
pub fn is_interrupt_line_pending<System: Plic>(
    line: InterruptNum,
) -> Result<bool, QueryInterruptLineError> {
    let plic_regs = System::plic_regs();

    if line > System::MAX_NUM {
        return Err(QueryInterruptLineError::BadParam);
    }

    Ok((plic_regs.interrupt_pending[line / 32].get() & (1u32 << (line % 32))) != 0)
}
