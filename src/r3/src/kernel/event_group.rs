//! Event groups
use core::{fmt, hash};

use super::{
    raw, raw_cfg, Cfg, GetEventGroupError, PollEventGroupError, UpdateEventGroupError,
    WaitEventGroupError, WaitEventGroupTimeoutError,
};
use crate::time::Duration;

pub use raw::{EventGroupBits, EventGroupWaitFlags};

// ----------------------------------------------------------------------------

/// Represents a single event group in a system.
///
/// An event group is a set of bits that can be updated and waited for to be
/// set.
///
/// This type is ABI-compatible with `System::`[`RawEventGroupId`][].
///
/// <div class="admonition-follows"></div>
///
/// > **Relation to Other Specifications:**
/// > event group (FreeRTOS), event group (Freescale MQX), `EventFlags` (Mbed
/// > OS), events (OSEK/VDX, assigned to each extended task), event (RT-Thread),
/// > event set (RTEMS, assigned to each task), Eventflag (μITRON4.0)
///
/// [`RawEventGroupId`]: raw::KernelEventGroup::RawEventGroupId
#[doc = include_str!("../common.md")]
#[repr(transparent)]
pub struct EventGroup<System: raw::KernelEventGroup>(System::RawEventGroupId);

impl<System: raw::KernelEventGroup> Clone for EventGroup<System> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<System: raw::KernelEventGroup> Copy for EventGroup<System> {}

impl<System: raw::KernelEventGroup> PartialEq for EventGroup<System> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<System: raw::KernelEventGroup> Eq for EventGroup<System> {}

impl<System: raw::KernelEventGroup> hash::Hash for EventGroup<System> {
    #[inline]
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        hash::Hash::hash(&self.0, state);
    }
}

impl<System: raw::KernelEventGroup> fmt::Debug for EventGroup<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("EventGroup").field(&self.0).finish()
    }
}

impl<System: raw::KernelEventGroup> EventGroup<System> {
    /// Construct a `EventGroup` from `RawEventGroupId`.
    ///
    /// # Safety
    ///
    /// The kernel can handle invalid IDs without a problem. However, the
    /// constructed `EventGroup` may point to an object that is not intended to be
    /// manipulated except by its creator. This is usually prevented by making
    /// `EventGroup` an opaque handle, but this safeguard can be circumvented by
    /// this method.
    #[inline]
    pub const unsafe fn from_id(id: System::RawEventGroupId) -> Self {
        Self(id)
    }

    /// Get the raw `RawEventGroupId` value representing this event group.
    #[inline]
    pub const fn id(self) -> System::RawEventGroupId {
        self.0
    }
}

impl<System: raw::KernelEventGroup> EventGroup<System> {
    /// Construct a `EventGroupDefiner` to define an event group in [a
    /// configuration function](crate#static-configuration).
    pub const fn define() -> EventGroupDefiner<System> {
        EventGroupDefiner::new()
    }

    /// Set the specified bits.
    #[inline]
    pub fn set(self, bits: EventGroupBits) -> Result<(), UpdateEventGroupError> {
        // Safety: `EventGroup` represents a permission to access the
        //         referenced object.
        unsafe { System::raw_event_group_set(self.0, bits) }
    }

    /// Clear the specified bits.
    #[inline]
    pub fn clear(self, bits: EventGroupBits) -> Result<(), UpdateEventGroupError> {
        // Safety: `EventGroup` represents a permission to access the
        //         referenced object.
        unsafe { System::raw_event_group_clear(self.0, bits) }
    }

    /// Get the currently set bits.
    #[inline]
    pub fn get(self) -> Result<EventGroupBits, GetEventGroupError> {
        // Safety: `EventGroup` represents a permission to access the
        //         referenced object.
        unsafe { System::raw_event_group_get(self.0) }
    }

    /// Wait for all or any of the specified bits to be set. Optionally, clear
    /// the specified bits.
    ///
    /// Returns the currently set bits. If `EventGroupWaitFlags::CLEAR` is
    /// specified, this method returns the bits before clearing.
    ///
    /// This system service may block. Therefore, calling this method is not
    /// allowed in [a non-waitable context] and will return `Err(BadContext)`.
    ///
    /// [a non-waitable context]: crate#contexts
    #[inline]
    pub fn wait(
        self,
        bits: EventGroupBits,
        flags: EventGroupWaitFlags,
    ) -> Result<EventGroupBits, WaitEventGroupError> {
        // Safety: `EventGroup` represents a permission to access the
        //         referenced object.
        unsafe { System::raw_event_group_wait(self.0, bits, flags) }
    }

    /// [`wait`](Self::wait) with timeout.
    #[inline]
    pub fn wait_timeout(
        self,
        bits: EventGroupBits,
        flags: EventGroupWaitFlags,
        timeout: Duration,
    ) -> Result<EventGroupBits, WaitEventGroupTimeoutError> {
        // Safety: `EventGroup` represents a permission to access the
        //         referenced object.
        unsafe { System::raw_event_group_wait_timeout(self.0, bits, flags, timeout) }
    }

    /// Non-blocking version of [`wait`](Self::wait). Returns immediately with
    /// [`PollEventGroupError::Timeout`] if the unblocking condition is not
    #[inline]
    /// satisfied.
    pub fn poll(
        self,
        bits: EventGroupBits,
        flags: EventGroupWaitFlags,
    ) -> Result<EventGroupBits, PollEventGroupError> {
        // Safety: `EventGroup` represents a permission to access the
        //         referenced object.
        unsafe { System::raw_event_group_poll(self.0, bits, flags) }
    }
}

// ----------------------------------------------------------------------------

/// The definer (static builder) for [`EventGroup`].
#[must_use = "must call `finish()` to complete definition"]
pub struct EventGroupDefiner<System: raw::KernelEventGroup> {
    inner: raw_cfg::EventGroupDescriptor<System>,
}

impl<System: raw::KernelEventGroup> EventGroupDefiner<System> {
    const fn new() -> Self {
        Self {
            inner: raw_cfg::EventGroupDescriptor {
                phantom: core::marker::PhantomData,
                initial_bits: 0,
                queue_order: raw::QueueOrder::TaskPriority,
            },
        }
    }

    /// Specify the initial bit pattern.
    pub const fn initial(self, initial: EventGroupBits) -> Self {
        Self {
            inner: raw_cfg::EventGroupDescriptor {
                initial_bits: initial,
                ..self.inner
            },
            ..self
        }
    }

    /// Specify how tasks are sorted in the wait queue of the event group.
    /// Defaults to [`QueueOrder::TaskPriority`] when unspecified.
    ///
    /// [`QueueOrder::TaskPriority`]: raw::QueueOrder::TaskPriority
    pub const fn queue_order(self, queue_order: raw::QueueOrder) -> Self {
        Self {
            inner: raw_cfg::EventGroupDescriptor {
                queue_order: queue_order,
                ..self.inner
            },
            ..self
        }
    }

    /// Complete the definition of an event group, returning a reference to the
    /// event group.
    pub const fn finish<C: ~const raw_cfg::CfgEventGroup<System = System>>(
        self,
        c: &mut Cfg<C>,
    ) -> EventGroup<System> {
        let id = c.raw().event_group_define(self.inner, ());
        unsafe { EventGroup::from_id(id) }
    }
}
