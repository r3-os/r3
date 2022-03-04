//! The low-level kernel static configuration interface to be implemented by a
//! kernel implementor.
//!
//! # General Structure
//!
//! This module includes the traits for a kernel-specific low-level configurator
//! type, which is used by [the kernel static configuration process][1] to
//! receive the specificiations of defined kernel objects and assign their IDs.
//! [`CfgBase`][] is the supertrait of all these traits and must be implemented
//! by the configurator type. It can optionally can implement other traits as
//! well if they can be supported.
//!
//! The `Cfg${Ty}` traits extend [`CfgBase`] by providing a
//! method named `${ty}_define` to define a kernel object of the corresponding
//! type (`${Ty}`). The method takes two parameters: `${Ty}Descriptor`
//! containing mandatory properties and `impl `[`Bag`] containing additional,
//! implementation-specific properties.
//!
//! The `${Ty}Descriptor` types contain mandatory (both for the consumers and
//! the implementors) properties of a kernel object to be created. They all
//! contain a `phantom: `[`PhantomInvariant`]`<System>` field to ensure they are
//! always parameterized and invariant over `System`.
//!
//! # Safety
//!
//! Most traits in this method are `unsafe trait` because they have to be
//! trustworthy to be able to build sound memory-safe abstractions on top of
//! them.
//!
//! # Stability
//!
//! This module is covered by [the kernel-side API stability guarantee][2].
//!
//! The trait paths in this module are covered by the application-side API
//! stability guarantee. Application code should only use these traits in trait
//! bounds and, to access the provided functionalities, should use the the
//! stable wrapper [outside this module](../index.html) instead.
//!
//! [1]: crate::kernel::cfg::KernelStatic
//! [2]: crate#stability
use crate::{bag::Bag, closure::Closure, kernel::raw, time::Duration, utils::PhantomInvariant};

/// The trait for all kernel-specific low-level configurator types, used by
/// [the kernel static configuration process][2].
///
/// # Safety
///
/// See [the module documentation][4].
///
/// # Stability
///
/// See [the module documentation][3].
///
/// [2]: crate::kernel::cfg::KernelStatic
/// [3]: self#stability
/// [4]: self#safety
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

/// A low-level configurator trait providing a method to define a
/// [task][1] in [the kernel static configuration process][2].
///
/// # Safety
///
/// See [the module documentation][4].
///
/// # Stability
///
/// See [the module documentation][3].
///
/// [1]: crate::kernel::StaticTask
/// [2]: crate::kernel::cfg::KernelStatic
/// [3]: self#stability
/// [4]: self#safety
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

/// A low-level configurator trait providing a method to define an
/// [event group][2] in [the kernel static configuration process][1].
///
/// # Safety
///
/// See [the module documentation][4].
///
/// # Stability
///
/// See [the module documentation][3].
///
/// [1]: crate::kernel::StaticEventGroup
/// [2]: crate::kernel::cfg::KernelStatic
/// [3]: self#stability
/// [4]: self#safety
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

/// A low-level configurator trait providing a method to define a
/// [mutex][2] in [the kernel static configuration process][1].
///
/// # Safety
///
/// See [the module documentation][4].
///
/// # Stability
///
/// See [the module documentation][3].
///
/// [1]: crate::kernel::StaticMutex
/// [2]: crate::kernel::cfg::KernelStatic
/// [3]: self#stability
/// [4]: self#safety
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

/// A low-level configurator trait providing a method to define a
/// [semaphore][2] in [the kernel static configuration process][1].
///
/// # Safety
///
/// See [the module documentation][4].
///
/// # Stability
///
/// See [the module documentation][3].
///
/// [1]: crate::kernel::StaticSemaphore
/// [2]: crate::kernel::cfg::KernelStatic
/// [3]: self#stability
/// [4]: self#safety
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

/// A low-level configurator trait providing a method to define a
/// [timwer][2] in [the kernel static configuration process][1].
///
/// # Safety
///
/// See [the module documentation][4].
///
/// # Stability
///
/// See [the module documentation][3].
///
/// [1]: crate::kernel::StaticTimer
/// [2]: crate::kernel::cfg::KernelStatic
/// [3]: self#stability
/// [4]: self#safety
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

/// A low-level configurator trait providing a method to define an
/// [interrupt line][2] in [the kernel static configuration process][1].
///
/// # Safety
///
/// See [the module documentation][4].
///
/// # Stability
///
/// See [the module documentation][3].
///
/// [1]: crate::kernel::InterruptLine
/// [2]: crate::kernel::cfg::KernelStatic
/// [3]: self#stability
/// [4]: self#safety
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
