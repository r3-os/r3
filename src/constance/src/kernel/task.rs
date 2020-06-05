//! Tasks
use core::marker::PhantomData;

use super::{ActivateTaskError, Id, Kernel};
use crate::utils::Init;

/// Identifies a task in a system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Task<System>(Id, PhantomData<System>);

impl<System: Kernel> Task<System> {
    /// Construct a `Task` from `Id`.
    pub const unsafe fn from_id(id: Id) -> Self {
        Self(id, PhantomData)
    }

    /// Get the raw `Id` value representing this task.
    pub const fn id(self) -> Id {
        self.0
    }

    /// Start the execution of the task.
    pub fn activate(self) -> Result<(), ActivateTaskError> {
        todo!()
    }
}

/// The state of a task.
///
/// This type isn't technically public but needs to be `pub` so that it can be
/// referred to by a macro.
#[doc(hidden)]
#[repr(C)]
pub struct TaskState<PortTaskState> {
    /// Place this at the beginning so that assembler code can refer to this
    /// easily.
    pub(super) port_task_state: PortTaskState,
}

impl<PortTaskState: Init> Init for TaskState<PortTaskState> {
    const INIT: Self = Self {
        port_task_state: Init::INIT,
    };
}

/// The static properties of a task.
///
/// This type isn't technically public but needs to be `pub` so that it can be
/// referred to by a macro.
#[doc(hidden)]
pub struct TaskAttr {}

impl Init for TaskAttr {
    const INIT: Self = Self {};
}
