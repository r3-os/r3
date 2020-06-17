use core::{fmt, hash, marker::PhantomData};

use super::{utils, EnableInterruptLineError, Kernel, SetInterruptLinePriorityError};

/// Numeric value used to identify interrupt lines.
///
/// The meaning of this value is defined by a port and target hardware. They
/// are not necessarily tightly packed from zero.
pub type InterruptNum = usize;

/// Priority value for an interrupt line.
pub type InterruptPriority = usize;

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
    /// interrupt handler. This method prevents this from happening and returns
    /// [`SetInterruptLinePriorityError::BadParam`].
    ///
    /// [a managed range]: crate#interrupt-handling-framework
    pub fn set_priority(
        self,
        _value: InterruptPriority,
    ) -> Result<(), SetInterruptLinePriorityError> {
        let _lock = utils::lock_cpu::<System>()?;
        todo!()
    }

    /// Set the priority of the interrupt line without checking if the new
    /// priority falls within [a managed range].
    ///
    /// [a managed range]: crate#interrupt-handling-framework
    pub unsafe fn set_priority_unchecked(
        self,
        _value: InterruptPriority,
    ) -> Result<(), SetInterruptLinePriorityError> {
        let _lock = utils::lock_cpu::<System>()?;
        todo!()
    }

    /// Enable the interrupt line.
    pub fn enable(self) -> Result<(), EnableInterruptLineError> {
        todo!()
    }

    /// Disable the interrupt line.
    pub fn disable(self) -> Result<(), EnableInterruptLineError> {
        todo!()
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
