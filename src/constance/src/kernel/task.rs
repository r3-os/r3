//! Tasks
use core::marker::PhantomData;

use super::{utils, ActivateTaskError, Id, Kernel};
use crate::utils::Init;

/// Represents a single task in a system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Task<System>(Id, PhantomData<System>);

impl<System> Task<System> {
    /// Construct a `Task` from `Id`.
    ///
    /// # Safety
    ///
    /// The kernel can handle invalid IDs without a problem. However, the
    /// constructed `Task` may point to an object that is not intended to be
    /// manipulated except by its creator. This is usually prevented by making
    /// `Task` an opaque handle, but this safeguard can be circumvented by
    /// this method.
    pub const unsafe fn from_id(id: Id) -> Self {
        Self(id, PhantomData)
    }
}

impl<System: Kernel> Task<System> {
    /// Get the raw `Id` value representing this task.
    pub const fn id(self) -> Id {
        self.0
    }

    /// Start the execution of the task.
    pub fn activate(self) -> Result<(), ActivateTaskError> {
        let _lock = utils::lock_cpu::<System>()?;

        todo!()
    }
}

/// *Task control block* - the state data of a task.
#[repr(C)]
pub struct TaskCb<PortTaskState> {
    /// Get a reference to `PortTaskState` in the task control block.
    ///
    /// This is guaranteed to be placed at the beginning of the struct so that
    /// assembler code can refer to this easily.
    pub port_task_state: PortTaskState,

    /// The static properties of the task.
    pub attr: &'static TaskAttr,

    pub(super) _force_int_mut: crate::utils::AssertSendSync<core::cell::UnsafeCell<()>>,
}

impl<PortTaskState: Init> Init for TaskCb<PortTaskState> {
    const INIT: Self = Self {
        port_task_state: Init::INIT,
        attr: &TaskAttr::INIT,
        _force_int_mut: crate::utils::AssertSendSync(core::cell::UnsafeCell::new(())),
    };
}

/// The static properties of a task.
pub struct TaskAttr {
    /// The entry point of the task.
    ///
    /// # Safety
    ///
    /// This is only meant to be used by a kernel port, as a task entry point,
    /// not by user code. Using this in other ways may cause an undefined
    /// behavior.
    pub entry_point: unsafe fn(usize),

    /// The parameter supplied for `entry_point`.
    pub entry_param: usize,
}

impl Init for TaskAttr {
    const INIT: Self = Self {
        entry_point: |_| {},
        entry_param: 0,
    };
}
