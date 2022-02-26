//! The low-level kernel static configuration interface to be implemented by a
//! kernel implementor.
//!
//! # General Structure
//!
//! TODO
//!
//! The `Cfg${TY}` traits extend [`CfgBase`] by providing a
//! method to define a kernel object of the corresponding type (`${TY}`). The
//! method takes two parameters: `${TY}Descriptor` containing mandatory
//! properties and `impl `[`Bag`] containing additional, implementation-specific
//! properties.
//!
//! The `${TY}Descriptor` types contain mandatory (both for the consumers and
//! the implementors) properties of a kernel object to be created. They all
//! contain a `phantom: `[`PhantomInvariant`]`<System>` field to ensure they are
//! always parameterized and invariant over `System`.
//!
//! # Safety
//!
//! Most traits in this method are `unsafe trait` because they have to be
//! trustworthy to be able to build sound memory-safe abstractions on top of
//! them.
use crate::{bag::Bag, closure::Closure, kernel::raw, time::Duration, utils::PhantomInvariant};

pub unsafe trait CfgBase {
    type System: raw::KernelBase;
    fn num_task_priority_levels(&mut self, new_value: usize);

    /// Register a combined [startup hook][1].
    ///
    /// The configuration system calls this exactly once for each built system.
    ///
    /// [1]: crate::kernel::hook::StartupHook
    fn startup_hook_define(&mut self, func: fn());
}

// The supertrait can't be `~const` due to [ref:const_supertraits]
pub unsafe trait CfgTask: CfgBase {
    fn task_define<Properties: ~const Bag>(
        &mut self,
        descriptor: TaskDescriptor<Self::System>,
        properties: Properties,
    ) -> <Self::System as raw::KernelBase>::RawTaskId;
}

/// The basic properties of a task.
#[derive(Debug)]
pub struct TaskDescriptor<System> {
    pub phantom: PhantomInvariant<System>,
    pub start: Closure,
    pub active: bool,
    pub priority: usize,
    pub stack_size: Option<usize>,
}

// The supertrait can't be `~const` due to [ref:const_supertraits]
pub unsafe trait CfgEventGroup: CfgBase
where
    Self::System: raw::KernelEventGroup,
{
    fn event_group_define<Properties: ~const Bag>(
        &mut self,
        descriptor: EventGroupDescriptor<Self::System>,
        properties: Properties,
    ) -> <Self::System as raw::KernelEventGroup>::RawEventGroupId;
}

/// The basic properties of an event group.
#[derive(Debug)]
pub struct EventGroupDescriptor<System> {
    pub phantom: PhantomInvariant<System>,
    pub initial_bits: raw::EventGroupBits,
    pub queue_order: raw::QueueOrder,
}

// The supertrait can't be `~const` due to [ref:const_supertraits]
pub unsafe trait CfgMutex: CfgBase
where
    Self::System: raw::KernelMutex,
{
    fn mutex_define<Properties: ~const Bag>(
        &mut self,
        descriptor: MutexDescriptor<Self::System>,
        properties: Properties,
    ) -> <Self::System as raw::KernelMutex>::RawMutexId;
}

/// The basic properties of a mutex.
#[derive(Debug)]
pub struct MutexDescriptor<System> {
    pub phantom: PhantomInvariant<System>,
    pub protocol: raw::MutexProtocol,
}

// The supertrait can't be `~const` due to [ref:const_supertraits]
pub unsafe trait CfgSemaphore: CfgBase
where
    Self::System: raw::KernelSemaphore,
{
    fn semaphore_define<Properties: ~const Bag>(
        &mut self,
        descriptor: SemaphoreDescriptor<Self::System>,
        properties: Properties,
    ) -> <Self::System as raw::KernelSemaphore>::RawSemaphoreId;
}

/// The basic properties of a semaphore.
#[derive(Debug)]
pub struct SemaphoreDescriptor<System> {
    pub phantom: PhantomInvariant<System>,
    pub initial: raw::SemaphoreValue,
    pub maximum: raw::SemaphoreValue,
    pub queue_order: raw::QueueOrder,
}

// The supertrait can't be `~const` due to [ref:const_supertraits]
pub unsafe trait CfgTimer: CfgBase
where
    Self::System: raw::KernelTimer,
{
    fn timer_define<Properties: ~const Bag>(
        &mut self,
        descriptor: TimerDescriptor<Self::System>,
        properties: Properties,
    ) -> <Self::System as raw::KernelTimer>::RawTimerId;
}

/// The basic properties of a timer.
#[derive(Debug)]
pub struct TimerDescriptor<System> {
    pub phantom: PhantomInvariant<System>,
    pub start: Closure,
    pub active: bool,
    pub delay: Option<Duration>,
    pub period: Option<Duration>,
}

// The supertrait can't be `~const` due to [ref:const_supertraits]
pub unsafe trait CfgInterruptLine: CfgBase
where
    Self::System: raw::KernelInterruptLine,
{
    fn interrupt_line_define<Properties: ~const Bag>(
        &mut self,
        descriptor: InterruptLineDescriptor<Self::System>,
        properties: Properties,
    );
}

/// The basic properties of an interrupt line.
#[derive(Debug)]
pub struct InterruptLineDescriptor<System> {
    pub phantom: PhantomInvariant<System>,
    pub line: raw::InterruptNum,
    pub priority: Option<raw::InterruptPriority>,
    pub start: Option<raw::InterruptHandlerFn>,
    pub enabled: bool,
}
