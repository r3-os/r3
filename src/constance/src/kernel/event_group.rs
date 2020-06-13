//! Event groups
use core::{fmt, hash, marker::PhantomData};

use super::{
    utils, GetEventGroupError, Id, Kernel, Port, UpdateEventGroupError, WaitEventGroupError,
};
use crate::utils::Init;

// TODO: Support changing `EventGroupBits`?
/// Unsigned integer type backing event groups.
pub type EventGroupBits = u32;

/// Represents a single event group in a system.
///
/// An event group is a set of bits that can be updated and waited for to be
/// set.
pub struct EventGroup<System>(Id, PhantomData<System>);

impl<System> Clone for EventGroup<System> {
    fn clone(&self) -> Self {
        Self(self.0, self.1)
    }
}

impl<System> Copy for EventGroup<System> {}

impl<System> PartialEq for EventGroup<System> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<System> Eq for EventGroup<System> {}

impl<System> hash::Hash for EventGroup<System> {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        hash::Hash::hash(&self.0, state);
    }
}

impl<System> fmt::Debug for EventGroup<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("EventGroup").field(&self.0).finish()
    }
}

bitflags::bitflags! {
    /// Options for [`EventGroup::wait`].
    pub struct EventGroupWaitFlags: u8 {
        /// Wait for all of the specified bits to be set.
        const ALL = 1 << 0;

        /// Clear the specified bits after waiting for them.
        const CLEAR = 1 << 1;
    }
}

impl<System> fmt::Debug for EventGroup<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("EventGroup").field(&self.0).finish()
    }
}

impl<System> EventGroup<System> {
    /// Construct a `EventGroup` from `Id`.
    ///
    /// # Safety
    ///
    /// The kernel can handle invalid IDs without a problem. However, the
    /// constructed `EventGroup` may point to an object that is not intended to be
    /// manipulated except by its creator. This is usually prevented by making
    /// `EventGroup` an opaque handle, but this safeguard can be circumvented by
    /// this method.
    pub const unsafe fn from_id(id: Id) -> Self {
        Self(id, PhantomData)
    }
}

impl<System: Kernel> EventGroup<System> {
    /// Get the raw `Id` value representing this event group.
    pub const fn id(self) -> Id {
        self.0
    }

    /// Set the specified bits.
    pub fn set(self, _bits: EventGroupBits) -> Result<(), UpdateEventGroupError> {
        let _lock = utils::lock_cpu::<System>()?;
        todo!()
    }

    /// Clear the specified bits.
    pub fn clear(self, _bits: EventGroupBits) -> Result<(), UpdateEventGroupError> {
        let _lock = utils::lock_cpu::<System>()?;
        todo!()
    }

    /// Get the currently set bits.
    pub fn get(self) -> Result<EventGroupBits, GetEventGroupError> {
        let _lock = utils::lock_cpu::<System>()?;
        todo!()
    }

    /// Wait for all or any of the specified bits to be set. Optionally, clear
    /// the specified bits.
    ///
    /// Returns the currently set bits. If `EventGroupWaitFlags::CLEAR` is
    /// specified, this method returns the bits before clearing.
    pub fn wait(
        self,
        _bits: EventGroupBits,
        _flags: EventGroupWaitFlags,
    ) -> Result<EventGroupBits, WaitEventGroupError> {
        let _lock = utils::lock_cpu::<System>()?;
        todo!()
    }
}

/// *Event group control block* - the state data of an event group.
#[repr(C)]
pub struct EventGroupCb<System: Port, EventGroupBits: 'static = self::EventGroupBits> {
    pub(super) bits: utils::CpuLockCell<System, EventGroupBits>,
}

impl<System: Port, EventGroupBits: Init + 'static> Init for EventGroupCb<System, EventGroupBits> {
    const INIT: Self = Self { bits: Init::INIT };
}

impl<System: Kernel, EventGroupBits: fmt::Debug + 'static> fmt::Debug
    for EventGroupCb<System, EventGroupBits>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EventGroupCb")
            .field("bits", &self.bits)
            .finish()
    }
}
