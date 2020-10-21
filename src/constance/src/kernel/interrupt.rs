use core::{fmt, hash, marker::PhantomData};

use super::{
    utils, ClearInterruptLineError, EnableInterruptLineError, Kernel, PendInterruptLineError, Port,
    QueryInterruptLineError, SetInterruptLinePriorityError,
};
use crate::utils::Init;

/// Numeric value used to identify interrupt lines.
///
/// The meaning of this value is defined by a port and target hardware. They
/// are not necessarily tightly packed from zero.
pub type InterruptNum = usize;

/// Priority value for an interrupt line.
pub type InterruptPriority = i16;

/// Refers to an interrupt line in a system.
pub struct InterruptLine<System>(InterruptNum, PhantomData<System>);

impl<System> Clone for InterruptLine<System> {
    fn clone(&self) -> Self {
        Self(self.0, self.1)
    }
}

impl<System> Copy for InterruptLine<System> {}

impl<System> PartialEq for InterruptLine<System> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<System> Eq for InterruptLine<System> {}

impl<System> hash::Hash for InterruptLine<System> {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        hash::Hash::hash(&self.0, state);
    }
}

impl<System> fmt::Debug for InterruptLine<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("InterruptLine").field(&self.0).finish()
    }
}

impl<System> InterruptLine<System> {
    /// Construct a `InterruptLine` from `InterruptNum`.
    pub const fn from_num(num: InterruptNum) -> Self {
        Self(num, PhantomData)
    }

    /// Get the raw `InterruptNum` value representing this interrupt line.
    pub const fn num(self) -> InterruptNum {
        self.0
    }
}

impl<System: Kernel> InterruptLine<System> {
    /// Set the priority of the interrupt line. The new priority must fall
    /// within [a managed range].
    ///
    /// Turning a managed interrupt handler into an unmanaged one is unsafe
    /// because the behavior of system calls is undefined inside an unmanaged
    /// interrupt handler. This method checks the new priority to prevent this
    /// from happening and returns [`SetInterruptLinePriorityError::BadParam`]
    /// if the operation is unsafe.
    ///
    /// [a managed range]: crate::kernel::PortInterrupts::MANAGED_INTERRUPT_PRIORITY_RANGE
    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    pub fn set_priority(
        self,
        value: InterruptPriority,
    ) -> Result<(), SetInterruptLinePriorityError> {
        let mut lock = utils::lock_cpu::<System>()?;

        // Deny a non-task context
        if !System::is_task_context() {
            return Err(SetInterruptLinePriorityError::BadContext);
        }

        // Deny unmanaged priority
        if !System::MANAGED_INTERRUPT_PRIORITY_RANGE.contains(&value) {
            return Err(SetInterruptLinePriorityError::BadParam);
        }

        // Safety: (1) Some of the preconditions of `set_priority_unchecked`,
        //         which are upheld by the caller.
        //         (2) A task context.
        unsafe { self.set_priority_unchecked_inner(value, lock.borrow_mut()) }
    }

    /// Set the priority of the interrupt line without checking if the new
    /// priority falls within [a managed range].
    ///
    /// [a managed range]: crate::kernel::PortInterrupts::MANAGED_INTERRUPT_PRIORITY_RANGE
    ///
    /// # Safety
    ///
    /// If a non-[unmanaged-safe] interrupt handler is attached to the interrupt
    /// line, changing the priority of the interrupt line to outside of the
    /// managed range (thus turning the handler into an unmanaged handler) may
    /// allow the interrupt handler to invoke an undefined behavior, for
    /// example, by making system calls, which are disallowed in an unmanaged
    /// interrupt handler.
    ///
    /// [unmanaged-safe]: crate::kernel::cfg::CfgInterruptHandlerBuilder::unmanaged
    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    pub unsafe fn set_priority_unchecked(
        self,
        value: InterruptPriority,
    ) -> Result<(), SetInterruptLinePriorityError> {
        let mut lock = utils::lock_cpu::<System>()?;

        // Deny a non-task context
        if !System::is_task_context() {
            return Err(SetInterruptLinePriorityError::BadContext);
        }

        // Safety: (1) Some of the preconditions of `set_priority_unchecked`,
        //         which are upheld by the caller.
        //         (2) A task context.
        unsafe { self.set_priority_unchecked_inner(value, lock.borrow_mut()) }
    }

    /// Like `set_priority_unchecked` but assumes a task context or a boot
    /// phase.
    ///
    /// # Safety
    ///
    /// In addition to `set_priority_unchecked`,
    #[inline]
    unsafe fn set_priority_unchecked_inner(
        self,
        value: InterruptPriority,
        _lock: utils::CpuLockGuardBorrowMut<System>,
    ) -> Result<(), SetInterruptLinePriorityError> {
        // Safety: (1) We are the kernel, so it's okay to call `Port`'s methods.
        //         (2) CPU Lock active
        unsafe { System::set_interrupt_line_priority(self.0, value) }
    }

    /// Enable the interrupt line.
    #[inline]
    pub fn enable(self) -> Result<(), EnableInterruptLineError> {
        // Safety: We are the kernel, so it's okay to call `Port`'s methods
        unsafe { System::enable_interrupt_line(self.0) }
    }

    /// Disable the interrupt line.
    #[inline]
    pub fn disable(self) -> Result<(), EnableInterruptLineError> {
        // Safety: We are the kernel, so it's okay to call `Port`'s methods
        unsafe { System::disable_interrupt_line(self.0) }
    }

    /// Set the pending flag of the interrupt line.
    #[inline]
    pub fn pend(self) -> Result<(), PendInterruptLineError> {
        // Safety: We are the kernel, so it's okay to call `Port`'s methods
        unsafe { System::pend_interrupt_line(self.0) }
    }

    /// Clear the pending flag of the interrupt line.
    #[inline]
    pub fn clear(self) -> Result<(), ClearInterruptLineError> {
        // Safety: We are the kernel, so it's okay to call `Port`'s methods
        unsafe { System::clear_interrupt_line(self.0) }
    }

    /// Read the pending flag of the interrupt line.
    #[inline]
    pub fn is_pending(self) -> Result<bool, QueryInterruptLineError> {
        // Safety: We are the kernel, so it's okay to call `Port`'s methods
        unsafe { System::is_interrupt_line_pending(self.0) }
    }

    // TODO: port-specific attributes
}

/// Represents a registered (second-level) interrupt handler in a system.
///
/// There are no operations defined for interrupt handlers, so this type
/// is only used for static configuration.
pub struct InterruptHandler<System>(PhantomData<System>);

impl<System> InterruptHandler<System> {
    pub(super) const fn new() -> Self {
        Self(PhantomData)
    }
}

/// Initialization parameter for an interrupt line.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct InterruptLineInit<System> {
    pub(super) line: InterruptLine<System>,
    pub(super) priority: InterruptPriority,
    pub(super) flags: InterruptLineInitFlags,
}

impl<System> Init for InterruptLineInit<System> {
    const INIT: Self = Self {
        line: InterruptLine::from_num(0),
        priority: Init::INIT,
        flags: InterruptLineInitFlags::empty(),
    };
}

bitflags::bitflags! {
    /// Flags for [`InterruptLineInit`].
    #[doc(hidden)]
    pub struct InterruptLineInitFlags: u32 {
        const ENABLE = 1 << 0;
        const SET_PRIORITY = 1 << 1;
    }
}

/// Initialization parameter for interrupt lines.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct InterruptAttr<System: Port> {
    pub line_inits: &'static [InterruptLineInit<System>],
}

impl<System: Kernel> InterruptAttr<System> {
    /// Initialize interrupt lines.
    ///
    /// # Safety
    ///
    /// This method may call `InterruptLine::set_priority_unchecked`. The caller
    /// is responsible for ensuring *unmanaged safety*.
    ///
    /// Can be called only during a boot phase.
    pub(super) unsafe fn init(&self, mut lock: utils::CpuLockGuardBorrowMut<System>) {
        for line_init in self.line_inits {
            if line_init
                .flags
                .contains(InterruptLineInitFlags::SET_PRIORITY)
            {
                // Safety: (1) The caller is responsible for ensuring unmanaged
                //             safety.
                //         (2) Boot phase
                unsafe {
                    line_init
                        .line
                        .set_priority_unchecked_inner(line_init.priority, lock.borrow_mut())
                        .unwrap()
                };
            }
            if line_init.flags.contains(InterruptLineInitFlags::ENABLE) {
                line_init.line.enable().unwrap();
            }
        }
    }
}
