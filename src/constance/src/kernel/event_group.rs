//! Event groups
use core::{
    fmt, hash,
    marker::PhantomData,
    sync::atomic::{AtomicU32, Ordering},
};

use super::{
    state, task, timeout, utils,
    wait::{WaitPayload, WaitQueue},
    BadIdError, GetEventGroupError, Id, Kernel, PollEventGroupError, Port, UpdateEventGroupError,
    WaitEventGroupError, WaitEventGroupTimeoutError,
};
use crate::{time::Duration, utils::Init};

// TODO: Support changing `EventGroupBits`?
/// Unsigned integer type backing event groups.
pub type EventGroupBits = u32;

pub type AtomicEventGroupBits = AtomicU32;

/// Represents a single event group in a system.
///
/// An event group is a set of bits that can be updated and waited for to be
/// set.
///
/// This type is ABI-compatible with [`Id`].
///
/// <div class="admonition-follows"></div>
///
/// > **Relation to Other Specifications:** Eventflag (μITRON4.0),
/// > `EventFlags` (Mbed OS), event group (FreeRTOS), event (RT-Thread),
/// > event group (Freescale MQX), event set (RTEMS, assigned to each task),
/// > events (OSEK/VDX, assigned to each extended task)
#[doc(include = "../common.md")]
#[repr(transparent)]
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

    /// Get the raw `Id` value representing this event group.
    pub const fn id(self) -> Id {
        self.0
    }
}

impl<System: Kernel> EventGroup<System> {
    fn event_group_cb(self) -> Result<&'static EventGroupCb<System>, BadIdError> {
        System::get_event_group_cb(self.0.get() - 1).ok_or(BadIdError::BadId)
    }

    /// Set the specified bits.
    pub fn set(self, bits: EventGroupBits) -> Result<(), UpdateEventGroupError> {
        let lock = utils::lock_cpu::<System>()?;
        let event_group_cb = self.event_group_cb()?;
        set(event_group_cb, lock, bits);
        Ok(())
    }

    /// Clear the specified bits.
    pub fn clear(self, bits: EventGroupBits) -> Result<(), UpdateEventGroupError> {
        let mut lock = utils::lock_cpu::<System>()?;
        let event_group_cb = self.event_group_cb()?;
        event_group_cb.bits.replace_with(&mut *lock, |b| *b & !bits);
        Ok(())
    }

    /// Get the currently set bits.
    pub fn get(self) -> Result<EventGroupBits, GetEventGroupError> {
        let lock = utils::lock_cpu::<System>()?;
        let event_group_cb = self.event_group_cb()?;
        Ok(event_group_cb.bits.get(&*lock))
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
    pub fn wait(
        self,
        bits: EventGroupBits,
        flags: EventGroupWaitFlags,
    ) -> Result<EventGroupBits, WaitEventGroupError> {
        let lock = utils::lock_cpu::<System>()?;
        state::expect_waitable_context::<System>()?;
        let event_group_cb = self.event_group_cb()?;

        wait(event_group_cb, lock, bits, flags)
    }

    /// [`wait`](Self::wait) with timeout.
    pub fn wait_timeout(
        self,
        bits: EventGroupBits,
        flags: EventGroupWaitFlags,
        timeout: Duration,
    ) -> Result<EventGroupBits, WaitEventGroupTimeoutError> {
        let time32 = timeout::time32_from_duration(timeout)?;
        let lock = utils::lock_cpu::<System>()?;
        state::expect_waitable_context::<System>()?;
        let event_group_cb = self.event_group_cb()?;

        wait_timeout(event_group_cb, lock, bits, flags, time32)
    }

    /// Non-blocking version of [`wait`](Self::wait). Returns immediately with
    /// [`PollEventGroupError::Timeout`] if the unblocking condition is not
    /// satisfied.
    pub fn poll(
        self,
        bits: EventGroupBits,
        flags: EventGroupWaitFlags,
    ) -> Result<EventGroupBits, PollEventGroupError> {
        let lock = utils::lock_cpu::<System>()?;
        let event_group_cb = self.event_group_cb()?;

        poll(event_group_cb, lock, bits, flags)
    }
}

/// *Event group control block* - the state data of an event group.
#[doc(hidden)]
pub struct EventGroupCb<System: Port, EventGroupBits: 'static = self::EventGroupBits> {
    pub(super) bits: utils::CpuLockCell<System, EventGroupBits>,

    pub(super) wait_queue: WaitQueue<System>,
}

impl<System: Port, EventGroupBits: Init + 'static> Init for EventGroupCb<System, EventGroupBits> {
    const INIT: Self = Self {
        bits: Init::INIT,
        wait_queue: Init::INIT,
    };
}

impl<System: Kernel, EventGroupBits: fmt::Debug + 'static> fmt::Debug
    for EventGroupCb<System, EventGroupBits>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EventGroupCb")
            .field("self", &(self as *const _))
            .field("bits", &self.bits)
            .field("wait_queue", &self.wait_queue)
            .finish()
    }
}

fn poll<System: Kernel>(
    event_group_cb: &'static EventGroupCb<System>,
    mut lock: utils::CpuLockGuard<System>,
    bits: EventGroupBits,
    flags: EventGroupWaitFlags,
) -> Result<EventGroupBits, PollEventGroupError> {
    if let Some(original_value) = poll_core(event_group_cb.bits.write(&mut *lock), bits, flags) {
        Ok(original_value)
    } else {
        Err(PollEventGroupError::Timeout)
    }
}

fn wait<System: Kernel>(
    event_group_cb: &'static EventGroupCb<System>,
    mut lock: utils::CpuLockGuard<System>,
    bits: EventGroupBits,
    flags: EventGroupWaitFlags,
) -> Result<EventGroupBits, WaitEventGroupError> {
    if let Some(original_value) = poll_core(event_group_cb.bits.write(&mut *lock), bits, flags) {
        Ok(original_value)
    } else {
        // The current state does not satify the wait condition. In this case,
        // start waiting. The wake-upper is responsible for using `poll_core`.
        let result = event_group_cb.wait_queue.wait(
            lock.borrow_mut(),
            WaitPayload::EventGroupBits {
                bits,
                flags,
                orig_bits: Init::INIT,
            },
        )?;

        // The original value will be copied to `orig_bits`
        if let WaitPayload::EventGroupBits { orig_bits, .. } = result {
            Ok(orig_bits.load(Ordering::Relaxed))
        } else {
            unreachable!()
        }
    }
}

fn wait_timeout<System: Kernel>(
    event_group_cb: &'static EventGroupCb<System>,
    mut lock: utils::CpuLockGuard<System>,
    bits: EventGroupBits,
    flags: EventGroupWaitFlags,
    time32: timeout::Time32,
) -> Result<EventGroupBits, WaitEventGroupTimeoutError> {
    if let Some(original_value) = poll_core(event_group_cb.bits.write(&mut *lock), bits, flags) {
        Ok(original_value)
    } else {
        // The current state does not satify the wait condition. In this case,
        // start waiting. The wake-upper is responsible for using `poll_core`.
        let result = event_group_cb.wait_queue.wait_timeout(
            lock.borrow_mut(),
            WaitPayload::EventGroupBits {
                bits,
                flags,
                orig_bits: Init::INIT,
            },
            time32,
        )?;

        // The original value will be copied to `orig_bits`
        if let WaitPayload::EventGroupBits { orig_bits, .. } = result {
            Ok(orig_bits.load(Ordering::Relaxed))
        } else {
            unreachable!()
        }
    }
}

/// Given a wait condition `(bits, flags)`, check if the current state of an
/// event group, `event_group_bits`, satisfies the wait condition.
///
/// If `event_group_bits` satisfies the wait condition, this function clears
/// some bits `event_group_bits` (if requested by `flags), and returns
/// `Some(original_value)`. Otherwise, it returns `None`.
fn poll_core(
    event_group_bits: &mut EventGroupBits,
    bits: EventGroupBits,
    flags: EventGroupWaitFlags,
) -> Option<EventGroupBits> {
    let success = if flags.contains(EventGroupWaitFlags::ALL) {
        (*event_group_bits & bits) == bits
    } else {
        (*event_group_bits & bits) != 0
    };

    if success {
        let original_value = *event_group_bits;
        if flags.contains(EventGroupWaitFlags::CLEAR) {
            *event_group_bits &= !bits;
        }
        Some(original_value)
    } else {
        None
    }
}

fn set<System: Kernel>(
    event_group_cb: &'static EventGroupCb<System>,
    mut lock: utils::CpuLockGuard<System>,
    added_bits: EventGroupBits,
) {
    let mut event_group_bits = event_group_cb.bits.get(&*lock);

    // Return early if no bits will change
    if (event_group_bits | added_bits) == event_group_bits {
        return;
    }

    event_group_bits |= added_bits;

    // Wake up tasks if their wake up conditions are now fulfilled.
    //
    // When waking up a task, some bits of `event_group_bits` might be cleared
    // if the waiter requests clearing bits. Clearing is handled by `poll_core`.
    let mut woke_up_any = false;

    event_group_cb
        .wait_queue
        .wake_up_all_conditional(lock.borrow_mut(), |wait_payload| match wait_payload {
            WaitPayload::EventGroupBits {
                bits,
                flags,
                orig_bits,
            } => {
                if let Some(orig) = poll_core(&mut event_group_bits, *bits, *flags) {
                    woke_up_any = true;
                    orig_bits.store(orig, Ordering::Relaxed);
                    true
                } else {
                    false
                }
            }
            _ => unreachable!(),
        });

    event_group_cb.bits.replace(&mut *lock, event_group_bits);

    if woke_up_any {
        task::unlock_cpu_and_check_preemption(lock);
    }
}
