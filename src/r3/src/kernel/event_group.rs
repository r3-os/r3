//! Event groups
use core::{fmt, hash};

use super::{
    raw, raw_cfg, Cfg, GetEventGroupError, PollEventGroupError, UpdateEventGroupError,
    WaitEventGroupError, WaitEventGroupTimeoutError,
};
use crate::time::Duration;

pub use raw::{EventGroupBits, EventGroupWaitFlags};

// ----------------------------------------------------------------------------

define_object! {
/// Represents a single owned event group in a system.
///
/// An event group is a set of bits that can be updated and waited for to be
/// set.
///
/// <div class="admonition-follows"></div>
///
/// > **Relation to Other Specifications:**
/// > event group (FreeRTOS), event group (Freescale MQX), `EventFlags` (Mbed
/// > OS), events (OSEK/VDX, assigned to each extended task), event (RT-Thread),
/// > event set (RTEMS, assigned to each task), Eventflag (μITRON4.0)
#[doc = include_str!("../common.md")]
pub struct EventGroup<System: _>(System::RawEventGroupId);

/// Represents a single borrowed event group in a system.
#[doc = include_str!("../common.md")]
pub struct EventGroupRef<System: raw::KernelEventGroup>(_);

pub trait EventGroupHandle {}
pub trait EventGroupMethods {}
}

impl<System: raw::KernelEventGroup> EventGroupRef<'_, System> {
    /// Construct a `EventGroupDefiner` to define an event group in [a
    /// configuration function](crate#static-configuration).
    pub const fn define() -> EventGroupDefiner<System> {
        EventGroupDefiner::new()
    }
}

/// The supported operations on [`EventGroupHandle`].
#[doc = include_str!("../common.md")]
pub trait EventGroupMethods: EventGroupHandle {
    /// Set the specified bits.
    #[inline]
    fn set(&self, bits: EventGroupBits) -> Result<(), UpdateEventGroupError> {
        // Safety: `EventGroup` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as raw::KernelEventGroup>::raw_event_group_set(self.id(), bits) }
    }

    /// Clear the specified bits.
    #[inline]
    fn clear(&self, bits: EventGroupBits) -> Result<(), UpdateEventGroupError> {
        // Safety: `EventGroup` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as raw::KernelEventGroup>::raw_event_group_clear(self.id(), bits) }
    }

    /// Get the currently set bits.
    #[inline]
    fn get(&self) -> Result<EventGroupBits, GetEventGroupError> {
        // Safety: `EventGroup` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as raw::KernelEventGroup>::raw_event_group_get(self.id()) }
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
    fn wait(
        &self,
        bits: EventGroupBits,
        flags: EventGroupWaitFlags,
    ) -> Result<EventGroupBits, WaitEventGroupError> {
        // Safety: `EventGroup` represents a permission to access the
        //         referenced object.
        unsafe {
            <Self::System as raw::KernelEventGroup>::raw_event_group_wait(self.id(), bits, flags)
        }
    }

    /// [`wait`](Self::wait) with timeout.
    #[inline]
    fn wait_timeout(
        &self,
        bits: EventGroupBits,
        flags: EventGroupWaitFlags,
        timeout: Duration,
    ) -> Result<EventGroupBits, WaitEventGroupTimeoutError> {
        // Safety: `EventGroup` represents a permission to access the
        //         referenced object.
        unsafe {
            <Self::System as raw::KernelEventGroup>::raw_event_group_wait_timeout(
                self.id(),
                bits,
                flags,
                timeout,
            )
        }
    }

    /// Non-blocking version of [`wait`](Self::wait). Returns immediately with
    /// [`PollEventGroupError::Timeout`] if the unblocking condition is not
    #[inline]
    /// satisfied.
    fn poll(
        &self,
        bits: EventGroupBits,
        flags: EventGroupWaitFlags,
    ) -> Result<EventGroupBits, PollEventGroupError> {
        // Safety: `EventGroup` represents a permission to access the
        //         referenced object.
        unsafe {
            <Self::System as raw::KernelEventGroup>::raw_event_group_poll(self.id(), bits, flags)
        }
    }
}

impl<T: EventGroupHandle> EventGroupMethods for T {}

// ----------------------------------------------------------------------------

/// The definer (static builder) for [`EventGroupRef`].
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
    ) -> EventGroupRef<'static, System> {
        let id = c.raw().event_group_define(self.inner, ());
        unsafe { EventGroupRef::from_id(id) }
    }
}
