use core::marker::PhantomData;

use r3::{
    kernel::{
        traits::KernelInterruptLine, ClearInterruptLineError, EnableInterruptLineError,
        InterruptNum, InterruptPriority, PendInterruptLineError, QueryInterruptLineError,
        SetInterruptLinePriorityError,
    },
    utils::Init,
};

use crate::{klock, KernelTraits, PortInterrupts, System};

unsafe impl<Traits: KernelTraits> r3::kernel::raw::KernelInterruptLine for System<Traits> {
    const MANAGED_INTERRUPT_PRIORITY_RANGE: core::ops::Range<InterruptPriority> =
        <Traits as PortInterrupts>::MANAGED_INTERRUPT_PRIORITY_RANGE;

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn interrupt_line_set_priority(
        this: InterruptNum,
        value: InterruptPriority,
    ) -> Result<(), SetInterruptLinePriorityError> {
        let mut _lock = klock::lock_cpu::<Traits>()?;

        // Deny a non-task context
        if !Traits::is_task_context() {
            return Err(SetInterruptLinePriorityError::BadContext);
        }

        // Safety: (1) We are the kernel, so it's okay to call `Port`'s methods.
        //         (2) CPU Lock active
        unsafe { Traits::set_interrupt_line_priority(this, value) }
    }

    #[inline]
    unsafe fn interrupt_line_enable(this: InterruptNum) -> Result<(), EnableInterruptLineError> {
        // Safety: We are the kernel, so it's okay to call `Port`'s methods
        unsafe { Traits::enable_interrupt_line(this) }
    }

    #[inline]
    unsafe fn interrupt_line_disable(this: InterruptNum) -> Result<(), EnableInterruptLineError> {
        // Safety: We are the kernel, so it's okay to call `Port`'s methods
        unsafe { Traits::disable_interrupt_line(this) }
    }

    #[inline]
    unsafe fn interrupt_line_pend(this: InterruptNum) -> Result<(), PendInterruptLineError> {
        // Safety: We are the kernel, so it's okay to call `Port`'s methods
        unsafe { Traits::pend_interrupt_line(this) }
    }

    #[inline]
    unsafe fn interrupt_line_clear(this: InterruptNum) -> Result<(), ClearInterruptLineError> {
        // Safety: We are the kernel, so it's okay to call `Port`'s methods
        unsafe { Traits::clear_interrupt_line(this) }
    }

    #[inline]
    unsafe fn interrupt_line_is_pending(
        this: InterruptNum,
    ) -> Result<bool, QueryInterruptLineError> {
        // Safety: We are the kernel, so it's okay to call `Port`'s methods
        unsafe { Traits::is_interrupt_line_pending(this) }
    }
}

/// Initialization parameter for an interrupt line.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct InterruptLineInit {
    pub(super) line: InterruptNum,
    pub(super) priority: InterruptPriority,
    pub(super) flags: InterruptLineInitFlags,
}

impl Init for InterruptLineInit {
    const INIT: Self = Self {
        line: 0,
        priority: Init::INIT,
        flags: InterruptLineInitFlags::empty(),
    };
}

bitflags::bitflags! {
    /// Flags for [`InterruptLineInit`].
    #[doc(hidden)]
    pub struct InterruptLineInitFlags: u32 {
        const ENABLE = 1 << 0;
        const SET_PRIORITY = 1 << 1;
    }
}

/// Initialization parameter for interrupt lines.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct InterruptAttr<Traits> {
    pub _phantom: PhantomData<Traits>,
    pub line_inits: &'static [InterruptLineInit],
}

impl<Traits: KernelTraits> InterruptAttr<Traits> {
    /// Initialize interrupt lines.
    ///
    /// # Safety
    ///
    /// This method may call `InterruptLine::set_priority_unchecked`. The caller
    /// is responsible for ensuring *unmanaged safety*.
    ///
    /// Can be called only during a boot phase.
    pub(super) unsafe fn init(&self, mut lock: klock::CpuLockTokenRefMut<Traits>) {
        for line_init in self.line_inits {
            if line_init
                .flags
                .contains(InterruptLineInitFlags::SET_PRIORITY)
            {
                // Safety: (1) The caller is responsible for ensuring unmanaged
                //             safety.
                //         (2) Boot phase
                unsafe {
                    <Traits as PortInterrupts>::set_interrupt_line_priority(
                        line_init.line,
                        line_init.priority,
                    )
                    .unwrap();
                }
            }
            if line_init.flags.contains(InterruptLineInitFlags::ENABLE) {
                unsafe { System::<Traits>::interrupt_line_enable(line_init.line).unwrap() };
            }
        }
    }
}
