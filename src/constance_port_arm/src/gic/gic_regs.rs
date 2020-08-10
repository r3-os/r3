#![allow(non_snake_case)]
use register::mmio::{ReadOnly, ReadWrite};

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

register::register_bitfields! {u32,
    pub GICC_CTLR [
        /// Enable for the signaling of Group 1 interrupts by the CPU interface
        /// to the connected processor.
        Enable OFFSET(0) NUMBITS(1) [
            Disable = 0,
            Enable = 1
        ]
    ]
}

register::register_bitfields! {u32,
    pub GICD_CTLR [
        /// Global enable for forwarding pending interrupts from the Distributor
        /// to the CPU interfaces
        Enable OFFSET(0) NUMBITS(1) [
            Disable = 0,
            Enable = 1
        ]
    ]
}

register::register_bitfields! {u32,
    pub GICD_TYPER [
        /// Indicates whether the GIC implements the Security Extensions.
        SecurityExtn OFFSET(10) NUMBITS(1) [
            Unimplemented = 0,
            Implemented = 1
        ],

        /// Indicates the number of implemented CPU interfaces. The number of
        /// implemented CPU interfaces is one more than the value of this field,
        /// for example if this field is 0b011, there are four CPU interfaces.
        /// If the GIC implements the Virtualization Extensions, this is also
        /// the number of virtual CPU interfaces.
        CPUNumber OFFSET(5) NUMBITS(3) [],

        /// Indicates the maximum number of interrupts that the GIC supports.
        /// If ITLinesNumber=N, the maximum number of interrupts is 32(N+1). The
        /// interrupt ID range is from 0 to (number of IDs – 1).
        ITLinesNumber OFFSET(0) NUMBITS(5) []
    ]
}
