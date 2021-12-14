//! Event groups
use core::fmt;
use r3::{
    kernel::{
        EventGroupBits, EventGroupWaitFlags, GetEventGroupError, PollEventGroupError,
        UpdateEventGroupError, WaitEventGroupError, WaitEventGroupTimeoutError,
    },
    time::Duration,
    utils::Init,
};

use crate::{
    error::NoAccessError,
    klock, state, task, timeout,
    wait::{WaitPayload, WaitQueue},
    KernelTraits, Port, System,
};

pub(super) type EventGroupId = crate::Id;

impl<Traits: KernelTraits> System<Traits> {
    fn event_group_cb(this: EventGroupId) -> Result<&'static EventGroupCb<Traits>, NoAccessError> {
        Traits::get_event_group_cb(this.get() - 1).ok_or(NoAccessError::NoAccess)
    }
}

unsafe impl<Traits: KernelTraits> r3::kernel::raw::KernelEventGroup for System<Traits> {
    type RawEventGroupId = EventGroupId;

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_event_group_set(
        this: EventGroupId,
        bits: EventGroupBits,
    ) -> Result<(), UpdateEventGroupError> {
        let lock = klock::lock_cpu::<Traits>()?;
        let event_group_cb = Self::event_group_cb(this)?;
        set(event_group_cb, lock, bits);
        Ok(())
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_event_group_clear(
        this: EventGroupId,
        bits: EventGroupBits,
    ) -> Result<(), UpdateEventGroupError> {
        let mut lock = klock::lock_cpu::<Traits>()?;
        let event_group_cb = Self::event_group_cb(this)?;
        event_group_cb.bits.replace_with(&mut *lock, |b| *b & !bits);
        Ok(())
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_event_group_get(
        this: EventGroupId,
    ) -> Result<EventGroupBits, GetEventGroupError> {
        let lock = klock::lock_cpu::<Traits>()?;
        let event_group_cb = Self::event_group_cb(this)?;
        Ok(event_group_cb.bits.get(&*lock))
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_event_group_wait(
        this: EventGroupId,
        bits: EventGroupBits,
        flags: EventGroupWaitFlags,
    ) -> Result<EventGroupBits, WaitEventGroupError> {
        let lock = klock::lock_cpu::<Traits>()?;
        state::expect_waitable_context::<Traits>()?;
        let event_group_cb = Self::event_group_cb(this)?;

        wait(event_group_cb, lock, bits, flags)
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_event_group_wait_timeout(
        this: EventGroupId,
        bits: EventGroupBits,
        flags: EventGroupWaitFlags,
        timeout: Duration,
    ) -> Result<EventGroupBits, WaitEventGroupTimeoutError> {
        let time32 = timeout::time32_from_duration(timeout)?;
        let lock = klock::lock_cpu::<Traits>()?;
        state::expect_waitable_context::<Traits>()?;
        let event_group_cb = Self::event_group_cb(this)?;

        wait_timeout(event_group_cb, lock, bits, flags, time32)
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_event_group_poll(
        this: EventGroupId,
        bits: EventGroupBits,
        flags: EventGroupWaitFlags,
    ) -> Result<EventGroupBits, PollEventGroupError> {
        let lock = klock::lock_cpu::<Traits>()?;
        let event_group_cb = Self::event_group_cb(this)?;

        poll(event_group_cb, lock, bits, flags)
    }
}

/// *Event group control block* - the state data of an event group.
#[doc(hidden)]
pub struct EventGroupCb<Traits: Port, EventGroupBits: 'static = self::EventGroupBits> {
    pub(super) bits: klock::CpuLockCell<Traits, EventGroupBits>,

    pub(super) wait_queue: WaitQueue<Traits>,
}

impl<Traits: Port, EventGroupBits: Init + 'static> Init for EventGroupCb<Traits, EventGroupBits> {
    const INIT: Self = Self {
        bits: Init::INIT,
        wait_queue: Init::INIT,
    };
}

impl<Traits: KernelTraits, EventGroupBits: fmt::Debug + 'static> fmt::Debug
    for EventGroupCb<Traits, EventGroupBits>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EventGroupCb")
            .field("self", &(self as *const _))
            .field("bits", &self.bits)
            .field("wait_queue", &self.wait_queue)
            .finish()
    }
}

fn poll<Traits: KernelTraits>(
    event_group_cb: &'static EventGroupCb<Traits>,
    mut lock: klock::CpuLockGuard<Traits>,
    bits: EventGroupBits,
    flags: EventGroupWaitFlags,
) -> Result<EventGroupBits, PollEventGroupError> {
    if let Some(original_value) = poll_core(event_group_cb.bits.write(&mut *lock), bits, flags) {
        Ok(original_value)
    } else {
        Err(PollEventGroupError::Timeout)
    }
}

fn wait<Traits: KernelTraits>(
    event_group_cb: &'static EventGroupCb<Traits>,
    mut lock: klock::CpuLockGuard<Traits>,
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
            Ok(orig_bits.read(&*lock).get())
        } else {
            unreachable!()
        }
    }
}

fn wait_timeout<Traits: KernelTraits>(
    event_group_cb: &'static EventGroupCb<Traits>,
    mut lock: klock::CpuLockGuard<Traits>,
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
            Ok(orig_bits.read(&*lock).get())
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

fn set<Traits: KernelTraits>(
    event_group_cb: &'static EventGroupCb<Traits>,
    mut lock: klock::CpuLockGuard<Traits>,
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
        .wake_up_all_conditional(lock.borrow_mut(), |wait_payload, lock| match wait_payload {
            WaitPayload::EventGroupBits {
                bits,
                flags,
                orig_bits,
            } => {
                if let Some(orig) = poll_core(&mut event_group_bits, *bits, *flags) {
                    woke_up_any = true;
                    orig_bits.read(&*lock).set(orig);
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
