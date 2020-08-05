use constance::kernel::InterruptNum;
use register::FieldValue;

use crate::gic_regs;

/// The options for [`use_gic!`].
pub trait GicOptions {
    /// The base address of GIC distributor registers.
    const GIC_DISTRIBUTOR_BASE: usize;

    /// The base address of GIC CPU interface registers.
    const GIC_CPU_BASE: usize;
}

/// Provides access to a system-global GIC instance. Implemented by [`use_gic!`].
pub unsafe trait Gic {
    #[doc(hidden)]
    /// Get `GicRegs` representing the system-global GIC instance.
    fn gic_regs() -> GicRegs;

    /// Get the number of supported interrupt lines.
    fn num_interrupt_lines() -> InterruptNum {
        let distributor = Self::gic_regs().distributor;
        let raw = distributor.TYPER.read(gic_regs::GICD_TYPER::ITLinesNumber);
        (raw as usize + 1) * 32
    }

    /// Set the trigger mode of the specified interrupt line.
    fn set_interrupt_line_trigger_mode(
        num: InterruptNum,
        mode: InterruptLineTriggerMode,
    ) -> Result<(), SetInterruptLineTriggerModeError> {
        let distributor = Self::gic_regs().distributor;

        // SGI (num = `0..16`) doesn't support changing trigger mode
        if num < 16 || num >= Self::num_interrupt_lines() {
            return Err(SetInterruptLineTriggerModeError::BadParam);
        }

        let int_config = mode as u32 * 2;
        distributor.ICFGR[num / 16].modify(FieldValue::<u32, ()>::new(
            0b10,
            (num % 16) * 2,
            int_config,
        ));

        Ok(())
    }
}

#[doc(hidden)]
/// Represents a GIC instance.
#[derive(Clone, Copy)]
pub struct GicRegs {
    pub(super) distributor: &'static gic_regs::GicDistributor,
    pub(super) cpu_interface: &'static gic_regs::GicCpuInterface,
}

/// Specifies the type of signal transition that pends an interrupt.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum InterruptLineTriggerMode {
    /// Asserts an interrupt whenever the interrupt signal level is active and
    /// deasserts whenever the level is not active.
    Level = 0,
    /// Asserts an interrupt upon detection of a rising edge of an interrupt
    /// signal.
    RisingEdge = 1,
}

pub enum SetInterruptLineTriggerModeError {
    /// The interrupt number is out of range.
    BadParam,
}

impl GicRegs {
    /// Construct a `GicRegs`.
    ///
    /// # Safety
    ///
    /// `GicOptions` should be configured correctly and the memory-mapped
    /// registers should be accessible.
    #[inline(always)]
    pub unsafe fn from_system<System: GicOptions>() -> Self {
        Self {
            distributor: unsafe {
                &*(System::GIC_DISTRIBUTOR_BASE as *const gic_regs::GicDistributor)
            },
            cpu_interface: unsafe { &*(System::GIC_CPU_BASE as *const gic_regs::GicCpuInterface) },
        }
    }
}
