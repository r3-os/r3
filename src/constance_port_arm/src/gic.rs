use constance::kernel::{
    ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
    PendInterruptLineError, QueryInterruptLineError, SetInterruptLinePriorityError,
};

use super::{gic_regs, Gic, GicRegs};

/// Implements [`crate::InterruptController::init`].
pub fn init<System: Gic>() {
    let GicRegs {
        distributor,
        cpu_interface,
    } = System::gic_regs();

    // Disable the distributor
    distributor
        .CTLR
        .modify(gic_regs::GICD_CTLR::Enable::Disable);

    let num_lines = System::num_interrupt_lines();

    // Disable all interrupt lines
    for r in &distributor.ICENABLE[0..(num_lines + 31) / 32] {
        r.set(0xffffffff);
    }

    // Clear all interrupt lines
    for r in &distributor.ICPEND[0..(num_lines + 31) / 32] {
        r.set(0xffffffff);
    }

    // Configure all interrupt lines as level-triggered
    for r in &distributor.ICFGR[0..(num_lines + 15) / 16] {
        r.set(0);
    }

    // Configure all interrupt lines to target CPU interface 0
    for r in &distributor.ITARGETS[0..(num_lines + 3) / 4] {
        r.set(0x01010101);
    }

    // Unmask all priorities in range `0..255`
    cpu_interface.PMR.set(0xff);

    // Allocate all priority bits for group priority
    cpu_interface.BPR.set(0);

    // Enable the distributor
    distributor.CTLR.modify(gic_regs::GICD_CTLR::Enable::Enable);

    // Enable the CPU interface
    cpu_interface
        .CTLR
        .modify(gic_regs::GICC_CTLR::Enable::Enable);
}

/// Implements [`crate::InterruptController::acknowledge_interrupt`].
#[inline]
pub fn acknowledge_interrupt<System: Gic>() -> Option<InterruptNum> {
    let cpu_interface = System::gic_regs().cpu_interface;
    let raw = cpu_interface.IAR.get();
    let interrupt_id = raw & 0x3ff;
    if interrupt_id == 0x3ff {
        None
    } else {
        Some(interrupt_id as _)
    }
}

/// Implements [`crate::InterruptController::end_interrupt`].
#[inline]
pub fn end_interrupt<System: Gic>(num: InterruptNum) {
    let cpu_interface = System::gic_regs().cpu_interface;
    cpu_interface.EOIR.set(num as _);
}

/// Implements [`constance::kernel::PortInterrupts::set_interrupt_line_priority`].
pub fn set_interrupt_line_priority<System: Gic>(
    line: InterruptNum,
    priority: InterruptPriority,
) -> Result<(), SetInterruptLinePriorityError> {
    let distributor = System::gic_regs().distributor;

    if line >= System::num_interrupt_lines() || priority < 0 || priority > 255 {
        return Err(SetInterruptLinePriorityError::BadParam);
    }

    distributor.IPRIORITY[line].set(priority as u8);

    Ok(())
}

/// Implements [`constance::kernel::PortInterrupts::enable_interrupt_line`].
pub fn enable_interrupt_line<System: Gic>(
    line: InterruptNum,
) -> Result<(), EnableInterruptLineError> {
    let distributor = System::gic_regs().distributor;

    // SGI (line `0..16`) does not support enabling/disabling.
    if line < 16 || line >= System::num_interrupt_lines() {
        return Err(EnableInterruptLineError::BadParam);
    }

    distributor.ISENABLE[line / 32].set(1 << (line % 32));

    Ok(())
}

/// Implements [`constance::kernel::PortInterrupts::disable_interrupt_line`].
pub fn disable_interrupt_line<System: Gic>(
    line: InterruptNum,
) -> Result<(), EnableInterruptLineError> {
    let distributor = System::gic_regs().distributor;

    // SGI (line `0..16`) does not support enabling/disabling.
    if line < 16 || line >= System::num_interrupt_lines() {
        return Err(EnableInterruptLineError::BadParam);
    }

    distributor.ICENABLE[line / 32].set(1 << (line % 32));

    Ok(())
}

/// Implements [`constance::kernel::PortInterrupts::pend_interrupt_line`].
pub fn pend_interrupt_line<System: Gic>(line: InterruptNum) -> Result<(), PendInterruptLineError> {
    let distributor = System::gic_regs().distributor;

    if line >= System::num_interrupt_lines() {
        return Err(PendInterruptLineError::BadParam);
    } else if line < 16 {
        distributor.SPENDSGIR[line].set(1);
    } else {
        distributor.ISPEND[line / 32].set(1 << (line % 32));
    }

    Ok(())
}

/// Implements [`constance::kernel::PortInterrupts::clear_interrupt_line`].
pub fn clear_interrupt_line<System: Gic>(
    line: InterruptNum,
) -> Result<(), ClearInterruptLineError> {
    let distributor = System::gic_regs().distributor;

    if line >= System::num_interrupt_lines() {
        return Err(ClearInterruptLineError::BadParam);
    } else if line < 16 {
        distributor.CPENDSGIR[line].set(1);
    } else {
        distributor.ICPEND[line / 32].set(1 << (line % 32));
    }

    Ok(())
}

/// Implements [`constance::kernel::PortInterrupts::is_interrupt_line_pending`].
pub fn is_interrupt_line_pending<System: Gic>(
    line: InterruptNum,
) -> Result<bool, QueryInterruptLineError> {
    let distributor = System::gic_regs().distributor;

    if line >= System::num_interrupt_lines() {
        return Err(QueryInterruptLineError::BadParam);
    }

    Ok((distributor.ISPEND[line / 32].get() & (1 << (line % 32))) != 0)
}
