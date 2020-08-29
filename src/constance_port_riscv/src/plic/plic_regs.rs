#![allow(non_snake_case)]
use register::mmio::{ReadOnly, ReadWrite};

/// RISC-V Platform-Level Interrupt Controller
///
/// <https://github.com/riscv/riscv-plic-spec/blob/master/riscv-plic.adoc>
#[repr(C)]
pub struct Plic {
    // +0x000000
    /// The interrupt priority for each interrupt source.
    ///
    /// > If PLIC supports Interrupt Priorities, then each PLIC interrupt source
    /// > can be assigned a priority by writing to its 32-bit memory-mapped
    /// > priority register. A priority value of 0 is reserved to mean ''never
    /// > interrupt'' and effectively disables the interrupt. Priority 1 is the
    /// > lowest active priority while the maximun level of priority depends on
    /// > PLIC implementation. Ties between global interrupts of the same
    /// > priority are broken by the Interrupt ID; interrupts with the lowest ID
    /// > have the highest effective priority.
    /// >
    /// > The base address of Interrupt Source Priority block within PLIC Memory
    /// > Map region is fixed at 0x000000.
    pub interrupt_priority: [ReadWrite<u32, ()>; 1024],

    // +0x001000
    /// The interrupt pending status of each interrupt source.
    ///
    /// > The current status of the interrupt source pending bits in the PLIC
    /// > core can be read from the pending array, organized as 32-bit register.
    /// > The pending bit for interrupt ID N is stored in bit (N mod 32) of word
    /// > (N/32). Bit 0 of word 0, which represents the non-existent interrupt
    /// > source 0, is hardwired to zero.
    /// >
    /// > A pending bit in the PLIC core can be cleared by setting the
    /// > associated enable bit then performing a claim.
    /// > The base address of Interrupt Pending Bits block within PLIC Memory
    /// > Map region is fixed at 0x001000.
    pub interrupt_pending: [ReadOnly<u32, ()>; 1024 / 32],

    _reserved1: [u32; 1024 - 1024 / 32],

    // +0x002000
    /// The enablement of interrupt source of each context.
    ///
    /// > Each global interrupt can be enabled by setting the corresponding bit
    /// > in the enables register. The enables registers are accessed as a
    /// > contiguous array of 32-bit registers, packed the same way as the
    /// > pending bits. Bit 0 of enable register 0 represents the non-existent
    /// > interrupt ID 0 and is hardwired to 0. PLIC has 15872 Interrupt Enable
    /// > blocks for the contexts. The context is referred to the specific
    /// > privilege mode in the specific Hart of specific RISC-V processor
    /// > instance. How PLIC organizes interrupts for the contexts (Hart and
    /// > privilege mode) is out of RISC-V PLIC specification scope, however it
    /// > must be spec-out in vendor’s PLIC specification.
    /// >
    /// > The base address of Interrupt Enable Bits block within PLIC Memory Map
    /// > region is fixed at 0x002000.
    pub interrupt_enable: [[ReadWrite<u32, ()>; 1024 / 32]; 15872],

    _reserved2: [u32; (0x200000 - 0x1f2000) / 4],

    // +0x200000
    pub ctxs: [PlicCtx; 15872],
}

#[repr(C)]
pub struct PlicCtx {
    /// The interrupt priority threshold of each context.
    ///
    /// > PLIC provides context based threshold register for the settings of a
    /// > interrupt priority threshold of each context. The threshold register
    /// > is a WARL field. The PLIC will mask all PLIC interrupts of a priority
    /// > less than or equal to threshold. For example, a`threshold` value of
    /// > zero permits all interrupts with non-zero priority.
    /// >
    /// > The base address of Priority Thresholds register block is located at
    /// > 4K alignement starts from offset 0x200000.
    pub priority_threshold: ReadWrite<u32, ()>,

    /// The claim/complete register.
    ///
    /// > # Interrupt Claim Process
    /// >
    /// > The PLIC can perform an interrupt claim by reading the
    /// > `claim/complete` register, which returns the ID of the highest
    /// > priority pending interrupt or zero if there is no pending interrupt. A
    /// > successful claim will also atomically clear the corresponding pending
    /// > bit on the interrupt source.
    /// >
    /// > The PLIC can perform a claim at any time and the claim operation is
    /// > not affected by the setting of the priority threshold register.
    /// > The Interrupt Claim Process register is context based and is located
    /// > at (4K alignement + 4) starts from offset 0x200000.
    /// >
    /// > # Interrupt Completion
    /// >
    /// > The PLIC signals it has completed executing an interrupt handler by
    /// > writing the interrupt ID it received from the claim to the
    /// > `claim/complete` register. The PLIC does not check whether the
    /// > completion ID is the same as the last claim ID for that target. If the
    /// > completion ID does not match an interrupt source that is currently
    /// > enabled for the target, the completion is silently ignored.
    /// > The Interrupt Completion registers are context based and located at
    /// > the same address with Interrupt Claim Process register, which is at
    /// > (4K alignement + 4) starts from offset 0x200000.
    pub claim_complete: ReadWrite<u32, ()>,

    _reserved: [u32; 0x400 - 2],
}
