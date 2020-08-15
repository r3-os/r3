/// The implementation of the Platform-Level Interrupt Controller driver.
use constance::kernel::{
    ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
    PendInterruptLineError, QueryInterruptLineError, SetInterruptLinePriorityError,
};

use crate::Plic;

/// Implements [`crate::InterruptController::init`].
pub fn init<System: Plic>() {
    // TODO
}

/// Implements [`crate::InterruptController::acknowledge_interrupt`].
#[inline]
pub fn acknowledge_interrupt<System: Plic>() -> Option<InterruptNum> {
    todo!()
}

/// Implements [`crate::InterruptController::end_interrupt`].
#[inline]
pub fn end_interrupt<System: Plic>(num: InterruptNum) {
    let _ = num;
    todo!()
}

/// Implements [`constance::kernel::PortInterrupts::set_interrupt_line_priority`].
pub fn set_interrupt_line_priority<System: Plic>(
    line: InterruptNum,
    priority: InterruptPriority,
) -> Result<(), SetInterruptLinePriorityError> {
    let _ = (line, priority);
    todo!()
}

/// Implements [`constance::kernel::PortInterrupts::enable_interrupt_line`].
pub fn enable_interrupt_line<System: Plic>(
    line: InterruptNum,
) -> Result<(), EnableInterruptLineError> {
    let _ = line;
    todo!()
}

/// Implements [`constance::kernel::PortInterrupts::disable_interrupt_line`].
pub fn disable_interrupt_line<System: Plic>(
    line: InterruptNum,
) -> Result<(), EnableInterruptLineError> {
    let _ = line;
    todo!()
}

/// Implements [`constance::kernel::PortInterrupts::pend_interrupt_line`].
pub fn pend_interrupt_line<System: Plic>(line: InterruptNum) -> Result<(), PendInterruptLineError> {
    let _ = line;
    todo!()
}

/// Implements [`constance::kernel::PortInterrupts::clear_interrupt_line`].
pub fn clear_interrupt_line<System: Plic>(
    line: InterruptNum,
) -> Result<(), ClearInterruptLineError> {
    let _ = line;
    todo!()
}

/// Implements [`constance::kernel::PortInterrupts::is_interrupt_line_pending`].
pub fn is_interrupt_line_pending<System: Plic>(
    line: InterruptNum,
) -> Result<bool, QueryInterruptLineError> {
    let _ = line;
    todo!()
}
