use constance::{
    kernel::{Port, PortToKernel, TaskCb},
    prelude::*,
    utils::Init,
};
use core::cell::UnsafeCell;

use super::ThreadingOptions;

/// Implemented on a system type by [`use_port!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_port!`].
pub unsafe trait PortInstance:
    Kernel + Port<PortTaskState = TaskState> + ThreadingOptions
{
    fn port_state() -> &'static State;
}

pub struct State {}

#[derive(Debug)]
#[repr(C)]
pub struct TaskState {
    // TODO
    sp: UnsafeCell<u32>,
}

unsafe impl Sync for TaskState {}

impl State {
    pub const fn new() -> Self {
        Self {}
    }
}

impl Init for TaskState {
    const INIT: Self = Self {
        sp: UnsafeCell::new(0),
    };
}

impl State {
    // TODO: Expose `port_boot` in case users don't want to use `use_startup!`
    pub unsafe fn port_boot<System: PortInstance>(&self) -> ! {
        unsafe { self.enter_cpu_lock::<System>() };

        // Safety: We are a port, so it's okay to call this
        unsafe {
            <System as PortToKernel>::boot();
        }
    }

    pub unsafe fn dispatch_first_task<System: PortInstance>(&'static self) -> ! {
        todo!()
    }

    pub unsafe fn yield_cpu<System: PortInstance>(&'static self) {
        todo!()
    }

    pub unsafe fn exit_and_dispatch<System: PortInstance>(
        &'static self,
        _task: &'static TaskCb<System>,
    ) -> ! {
        todo!()
    }

    #[inline(always)]
    pub unsafe fn enter_cpu_lock<System: PortInstance>(&self) {
        // TODO: support unmanaged interrupts
        unsafe { llvm_asm!("cpsid i"::::"volatile") };
    }

    #[inline(always)]
    pub unsafe fn leave_cpu_lock<System: PortInstance>(&'static self) {
        unsafe { llvm_asm!("cpsie i"::::"volatile") };
    }

    pub unsafe fn initialize_task_state<System: PortInstance>(
        &self,
        _task: &'static TaskCb<System>,
    ) {
        todo!()
    }

    #[inline(always)]
    pub fn is_cpu_lock_active<System: PortInstance>(&self) -> bool {
        let cpsr: u32;
        unsafe { llvm_asm!("mrs $0, cpsr":"=r"(cpsr)) };
        (cpsr & (1 << 7)) != 0
    }

    pub fn is_task_context<System: PortInstance>(&self) -> bool {
        todo!()
    }
}

/// Used by `use_port!`
pub const fn validate<System: PortInstance>() {}
