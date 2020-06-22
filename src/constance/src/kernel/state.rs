use super::{BadContextError, Kernel};

/// If the current context is not waitable, return `Err(BadContext)`.
pub(super) fn expect_waitable_context<System: Kernel>() -> Result<(), BadContextError> {
    if System::is_interrupt_context() {
        Err(BadContextError::BadContext)
    } else {
        // TODO: priority boost
        Ok(())
    }
}
