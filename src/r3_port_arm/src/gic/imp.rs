//! Under the hood
use r3_core::kernel::{
    ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
    PendInterruptLineError, QueryInterruptLineError, SetInterruptLinePriorityError,
};
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

use super::{
    cfg::{Gic, GicOptions},
    gic_regs,
};

#[doc(hidden)]
/// Represents a GIC instance.
#[derive(Clone, Copy)]
pub struct GicRegs {
    pub(super) distributor: &'static gic_regs::GicDistributor,
    pub(super) cpu_interface: &'static gic_regs::GicCpuInterface,
}

impl GicRegs {
    /// Construct a `GicRegs`.
    ///
    /// # Safety
    ///
    /// `GicOptions` should be configured correctly and the memory-mapped
    /// registers should be accessible.
    #[inline(always)]
    pub unsafe fn from_system_traits<Traits: GicOptions>() -> Self {
        Self {
            distributor: unsafe {
                &*(Traits::GIC_DISTRIBUTOR_BASE as *const gic_regs::GicDistributor)
            },
            cpu_interface: unsafe { &*(Traits::GIC_CPU_BASE as *const gic_regs::GicCpuInterface) },
        }
    }
}

/// Implements [`crate::InterruptController::init`].
pub fn init<Traits: Gic>() {
    let GicRegs {
        distributor,
        cpu_interface,
    } = Traits::gic_regs();

    // Disable the distributor
    distributor
        .CTLR
        .modify(gic_regs::GICD_CTLR::Enable::Disable);

    let num_lines = Traits::num_interrupt_lines();

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

    // Deactivate any active interrupts
    while let Some(x) = acknowledge_interrupt::<Traits>() {
        end_interrupt::<Traits>(x);
    }

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
pub fn acknowledge_interrupt<Traits: Gic>() -> Option<InterruptNum> {
    let cpu_interface = Traits::gic_regs().cpu_interface;
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
pub fn end_interrupt<Traits: Gic>(num: InterruptNum) {
    let cpu_interface = Traits::gic_regs().cpu_interface;
    cpu_interface.EOIR.set(num as _);
}

/// Implements [`r3_kernel::PortInterrupts::set_interrupt_line_priority`].
pub fn set_interrupt_line_priority<Traits: Gic>(
    line: InterruptNum,
    priority: InterruptPriority,
) -> Result<(), SetInterruptLinePriorityError> {
    let distributor = Traits::gic_regs().distributor;

    if line >= Traits::num_interrupt_lines() || priority < 0 || priority > 255 {
        return Err(SetInterruptLinePriorityError::BadParam);
    }

    distributor.IPRIORITY[line].set(priority as u8);

    Ok(())
}

/// Implements [`r3_kernel::PortInterrupts::enable_interrupt_line`].
pub fn enable_interrupt_line<Traits: Gic>(
    line: InterruptNum,
) -> Result<(), EnableInterruptLineError> {
    let distributor = Traits::gic_regs().distributor;

    // SGI (line `0..16`) does not support enabling/disabling.
    if line < 16 || line >= Traits::num_interrupt_lines() {
        return Err(EnableInterruptLineError::BadParam);
    }

    distributor.ISENABLE[line / 32].set(1 << (line % 32));

    Ok(())
}

/// Implements [`r3_kernel::PortInterrupts::disable_interrupt_line`].
pub fn disable_interrupt_line<Traits: Gic>(
    line: InterruptNum,
) -> Result<(), EnableInterruptLineError> {
    let distributor = Traits::gic_regs().distributor;

    // SGI (line `0..16`) does not support enabling/disabling.
    if line < 16 || line >= Traits::num_interrupt_lines() {
        return Err(EnableInterruptLineError::BadParam);
    }

    distributor.ICENABLE[line / 32].set(1 << (line % 32));

    Ok(())
}

/// Implements [`r3_kernel::PortInterrupts::pend_interrupt_line`].
pub fn pend_interrupt_line<Traits: Gic>(line: InterruptNum) -> Result<(), PendInterruptLineError> {
    let distributor = Traits::gic_regs().distributor;

    if line >= Traits::num_interrupt_lines() {
        return Err(PendInterruptLineError::BadParam);
    } else if line < 16 {
        distributor.SPENDSGIR[line].set(1);
    } else {
        distributor.ISPEND[line / 32].set(1 << (line % 32));
    }

    Ok(())
}

/// Implements [`r3_kernel::PortInterrupts::clear_interrupt_line`].
pub fn clear_interrupt_line<Traits: Gic>(
    line: InterruptNum,
) -> Result<(), ClearInterruptLineError> {
    let distributor = Traits::gic_regs().distributor;

    if line >= Traits::num_interrupt_lines() {
        return Err(ClearInterruptLineError::BadParam);
    } else if line < 16 {
        distributor.CPENDSGIR[line].set(1);
    } else {
        distributor.ICPEND[line / 32].set(1 << (line % 32));
    }

    Ok(())
}

/// Implements [`r3_kernel::PortInterrupts::is_interrupt_line_pending`].
pub fn is_interrupt_line_pending<Traits: Gic>(
    line: InterruptNum,
) -> Result<bool, QueryInterruptLineError> {
    let distributor = Traits::gic_regs().distributor;

    if line >= Traits::num_interrupt_lines() {
        return Err(QueryInterruptLineError::BadParam);
    }

    Ok((distributor.ISPEND[line / 32].get() & (1 << (line % 32))) != 0)
}
