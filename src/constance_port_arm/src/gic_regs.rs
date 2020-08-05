#![allow(non_snake_case)]
use register::mmio::{ReadOnly, ReadWrite};

mod gicc_ctlr;
mod gicd_ctlr;
mod gicd_typer;
pub use self::gicc_ctlr::*;
pub use self::gicd_ctlr::*;
pub use self::gicd_typer::*;

#[repr(C)]
pub struct GicDistributor {
    /// Distributor Control Register
    pub CTLR: ReadWrite<u32, GICD_CTLR::Register>,
    /// Interrupt Controller Type Register
    pub TYPER: ReadOnly<u32, GICD_TYPER::Register>,
    /// Distributor Implementer Identification Register
    pub IIDR: ReadOnly<u32>,
    _reserved1: [u32; 5],
    _implementation_defined1: [u32; 8],
    _reserved2: [u32; 16],
    /// Interrupt Group Registers
    pub IGROUPR: ReadWrite<u32>,
    _reserved3: [u32; 31],
    /// Interrupt Set-Enable Registers
    pub ISENABLE: [ReadWrite<u32>; 32],
    /// Interrupt Clear-Enable Registers
    pub ICENABLE: [ReadWrite<u32>; 32],
    /// Interrupt Set-Pending Registers
    pub ISPEND: [ReadWrite<u32>; 32],
    /// Interrupt Clear-Pending Registers
    pub ICPEND: [ReadWrite<u32>; 32],
    /// Interrupt Set-Active Registers
    pub ISACTIVE: [ReadWrite<u32>; 32],
    /// Interrupt Clear-Active Registers
    pub ICACTIVE: [ReadWrite<u32>; 32],
    /// Interrupt Priority Registers
    pub IPRIORITY: [ReadWrite<u8>; 1024],
    /// Interrupt Processor Targets Registers
    pub ITARGETS: [ReadWrite<u32>; 255],
    _reserved5: u32,
    /// Interrupt Configuration Registers
    pub ICFGR: [ReadWrite<u32>; 64],
    _implementation_defined2: [u32; 64],
    /// Non-secure Access Control Registers, optional
    pub NSACR: [ReadWrite<u32>; 64],
    /// Software Generated Interrupt Register
    pub SGIR: ReadWrite<u32>,
    _reserved6: [u32; 3],
    /// SGI Clear-Pending Registers
    pub CPENDSGIR: [ReadWrite<u8>; 16],
    /// SGI Set-Pending Registers
    pub SPENDSGIR: [ReadWrite<u8>; 16],
    _reserved7: [u32; 40],
    _implementation_defined3: [u32; 12],
}

#[repr(C)]
pub struct GicCpuInterface {
    /// CPU Interface Control Register
    pub CTLR: ReadWrite<u32, GICC_CTLR::Register>,
    /// Interrupt Priority Mask Register
    pub PMR: ReadWrite<u32>,
    /// Binary Point Register
    pub BPR: ReadWrite<u32>,
    /// Interrupt Acknowledge Register
    pub IAR: ReadWrite<u32>,
    /// End of Interrupt Register
    pub EOIR: ReadWrite<u32>,
    /// Running Priority Register
    pub RPR: ReadWrite<u32>,
    /// Highest Priority Pending Interrupt Register
    pub HPPIR: ReadWrite<u32>,
    /// Aliased Binary Point Register
    pub ABPR: ReadWrite<u32>,
    /// Aliased Interrupt Acknowledge Register
    pub AIAR: ReadWrite<u32>,
    /// Aliased End of Interrupt Register
    pub AEOIR: ReadWrite<u32>,
    /// Aliased Highest Priority Pending Interrupt Register
    pub AHPPIR: ReadWrite<u32>,
}
