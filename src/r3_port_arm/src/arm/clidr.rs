register::register_bitfields! {u32,
    pub CLIDR [
        Ctype1 OFFSET(0) NUMBITS(3) [
            NoCache = 0b000,
            Instruction = 0b001,
            Data = 0b010,
            InstructionAndData = 0b011,
            Unified = 0b100
        ],
        // Ctype2 OFFSET(3) NUMBITS(3) []
        // Ctype3 OFFSET(6) NUMBITS(3) []
        // Ctype4 OFFSET(9) NUMBITS(3) []
        // Ctype5 OFFSET(12) NUMBITS(3) []
        // Ctype6 OFFSET(15) NUMBITS(3) []
        // Ctype7 OFFSET(18) NUMBITS(3) []

        /// Level of Coherency for the cache hierarchy.
        LoC OFFSET(24) NUMBITS(3) []
    ]
}

/// Cache Level ID Register
pub const CLIDR: CLIDRAccessor = CLIDRAccessor;
pub struct CLIDRAccessor;

impl register::cpu::RegisterReadOnly<u32, CLIDR::Register> for CLIDRAccessor {
    sys_coproc_read_raw!(u32, [p15, c0, 1, c0, 1]);
}
