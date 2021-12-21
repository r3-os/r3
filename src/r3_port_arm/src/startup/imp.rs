//! Provides a standard startup and entry code implementation.
use core::arch::asm;
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

use crate::{arm, startup::cfg::MemoryRegionAttributes, EntryPoint, StartupOptions};

#[repr(align(4096), C)]
struct VectorTable {
    _trampolines: [u32; 8],
    _targets: [unsafe extern "C" fn() -> !; 8],
}

impl VectorTable {
    const fn new<Traits: EntryPoint>() -> Self {
        Self {
            // trampolines[N]:
            //      ldr pc, [pc, #24]   ; targets + N * 4
            _trampolines: [0xe59ff018; 8],
            // trampolines[N]
            _targets: [
                unhandled_exception_handler,
                undefined_instruction_handler,
                supervisor_call_handler,
                prefetch_abort_handler,
                data_abort_handler,
                unhandled_exception_handler,
                Traits::IRQ_ENTRY,
                fiq_handler,
            ],
        }
    }
}

#[naked]
pub extern "C" fn start<Traits: EntryPoint + StartupOptions>() {
    unsafe {
        // Set the stack pointer before calling Rust code
        asm!("
            ldr r0, =_stack_start

            # Set the stack for IRQ mode
            msr cpsr_c, #0xd2
            mov sp, r0

            # Set the stack for FIQ mode
            msr cpsr_c, #0xd1
            mov sp, r0

            # Set the stack for Abort mode
            msr cpsr_c, #0xd7
            mov sp, r0

            # Set the stack for Undefined Instruction mode
            msr cpsr_c, #0xdb
            mov sp, r0

            # Back to Supervisor mode, (IRQ and FIQ both masked, Arm instruction
            # set) set the stack for Supervisor mode
            msr cpsr_c, #0xd3
            mov sp, r0

            b {reset_handler1}
            ",
            reset_handler1 = sym reset_handler1::<Traits>,
            options(noreturn),
        );
    }
}

extern "C" fn reset_handler1<Traits: EntryPoint + StartupOptions>() {
    arm::SCTLR.modify(
        // Disable data and unified caches
        arm::SCTLR::C::Disable +
        // Disable MMU
        arm::SCTLR::M::Disable,
    );

    // Invalidate instruction cache
    arm::ICIALLU.set(0);

    // Invalidate branch prediction array
    arm::BPIALL.set(0);

    // Invalidate data and unified cache
    // This part is based on the section “8.9.1. Example code for cache
    // maintenance operations” of Cortex-A Series Programmers Guide 4.0.
    //
    // Level of Coherency: “This field defines the last level of cache that must
    // be cleaned or invalidated when cleaning or invalidating to the point of
    // coherency.”
    let clidr = arm::CLIDR.extract();
    let level_of_coherency = clidr.read(arm::CLIDR::LoC);
    for level in 0..level_of_coherency {
        let cache_type = (clidr.get() >> (level * 3)) & 0b111;

        // Does this cache level include a data or unified cache?
        if cache_type >= 2 {
            // Level = level, InD = 0
            // Use `isb` to make sure the change to CSSELR takes effect.
            arm::CSSELR.set(level * 2);
            unsafe { asm!("isb") };

            let cssidr = arm::CCSIDR.extract();
            let log2_line_size = cssidr.read(arm::CCSIDR::LineSize) + 4;
            let max_way_index = cssidr.read(arm::CCSIDR::Associativity);
            let max_set_index = cssidr.read(arm::CCSIDR::NumSets);

            let way_offset = max_way_index.leading_zeros();

            for way in (0..=max_way_index).rev() {
                for set in (0..=max_set_index).rev() {
                    let set_way = (level << 1) | (way << way_offset) | (set << log2_line_size);

                    // Invalidate by set/way
                    arm::DCISW.set(set_way);
                }
            }
        }
    }

    // Configure MMU
    let page_table_ptr = (&Traits::PAGE_TABLE) as *const _ as usize;
    arm::TTBCR.write(
        // Only use `TTBR0`
        arm::TTBCR::N.val(0) +
        // Security Extensions: Don't fault on TTBR0 TLB miss
        arm::TTBCR::PD0::Default +
        // Security Extensions: Don't fault on TTBR1 TLB miss
        arm::TTBCR::PD1::Default +
        // Disable extended address
        arm::TTBCR::EAE::CLEAR,
    );
    arm::DACR.write(arm::DACR::D0::Client);
    arm::TTBR0.write(
        // Table walk is Cachable
        arm::TTBR0::C::SET +
        // Table walk is Non-shareable
        arm::TTBR0::S::CLEAR +
        // Table walk is Normal memory, Outer Write-Back Write-Allocate cachable
        arm::TTBR0::RGN::OuterWriteBackWriteAllocate +
        // Table walk is Outer Shareable (ignored because `S == 0`)
        arm::TTBR0::NOS::OuterShareable +
        // Page table
        arm::TTBR0::BASE.val(page_table_ptr as u32 >> 14),
    );

    // Invalidate TLB
    arm::TLBIALL.set(0);

    // DSB causes completion of all preceding cache and branch predictor
    // mantenance operations. ISB causes the effect to be visible to all
    // subsequent instructions.
    unsafe { asm!("dsb") };
    unsafe { asm!("isb") };

    arm::SCTLR.modify(
        // Enable data and unified caches
        arm::SCTLR::C::Enable +
        // Enable instruction caches
        arm::SCTLR::I::Enable +
        // Enable MMU
        arm::SCTLR::M::Enable +
        // Specify the vector table base address
        if Traits::VECTOR_HIGH {
            arm::SCTLR::V::High
        } else {
            arm::SCTLR::V::Low
        } +
        // Enable alignment fault checking
        arm::SCTLR::A::Enable +
        // Enable branch prediction
        arm::SCTLR::Z::Enable +
        // Disable access flags in a page table
        arm::SCTLR::AFE::Disable +
        // Exceptions are taken in Arm state
        arm::SCTLR::TE::Arm,
    );

    // Ensure the changes made to `SCTLR` here take effect immediately
    unsafe { asm!("isb") };

    extern "C" {
        // These symbols come from `link.x`
        static mut __sbss: u32;
        static mut __ebss: u32;
    }

    // Initialize RAM
    unsafe {
        r0::zero_bss(&mut __sbss, &mut __ebss);
    }

    unsafe { Traits::start() };
}

// FIXME: `pub` in these functions is to work around an ICE issue
//        `error: internal compiler error: src/librustc_mir/monomorphize/
//         collector.rs:802:9: cannot create local mono-item for DefId(4:27 ~
//         r3_port_arm[c8c4]::startup[0]::unhandled_exception_handler[0])`
pub extern "C" fn unhandled_exception_handler() -> ! {
    panic!("reserved exception");
}

pub extern "C" fn undefined_instruction_handler() -> ! {
    panic!("undefined instruction");
}

pub extern "C" fn supervisor_call_handler() -> ! {
    panic!("unexpected supervisor call");
}

pub extern "C" fn prefetch_abort_handler() -> ! {
    panic!("prefetch abort");
}

pub extern "C" fn data_abort_handler() -> ! {
    panic!("data abort");
}

pub extern "C" fn fiq_handler() -> ! {
    panic!("unexpecte fiq");
}

// Page table generation
// -----------------------------------------------------------------------

/// The extension trait for deriving static data based on `StartupOptions`.
trait StartupExt {
    const VECTOR_TABLE: VectorTable;

    /// The vector table base address. `false` = `0x00000000`, `true` =
    /// `0xffff0000`.
    const VECTOR_HIGH: bool;

    /// The page table for `0x000xxxxx` or `0xfffxxxxx` (depending on
    /// `VECTOR_HIGH`).
    const VECTOR_PAGE_TABLE: SecondLevelPageTable;

    const PAGE_TABLE: FirstLevelPageTable;
}

impl<T: StartupOptions + EntryPoint> StartupExt for T {
    const VECTOR_TABLE: VectorTable = VectorTable::new::<Self>();

    const VECTOR_HIGH: bool = {
        // Find an unmapped location. Prefer `0xffff0000` so that we can catch
        // null pointer dereferences.
        if !memory_map_maps_va::<T>(0xffff0000) {
            true
        } else if !memory_map_maps_va::<T>(0x00000000) {
            false
        } else {
            panic!(
                "couldn't determine the vector table base address. at least \
                one of 0x0000_0000 and 0xffff_0000 must be left unmapped \
                by `StartupOptions::MEMORY_MAP`."
            );
        }
    };

    const VECTOR_PAGE_TABLE: SecondLevelPageTable = {
        let mut table = SecondLevelPageTable {
            entries: [SecondLevelPageEntry::fault(); 256],
        };

        let vector_table = &Self::VECTOR_TABLE as *const _ as *mut u8;

        let attr = MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE
            .with_sharable(false)
            .with_writable(false)
            .with_executable(true);

        if Self::VECTOR_HIGH {
            // This second-level table is for `0xfffxxxxx`.
            // Make a page entry for the virtual address `0xffff0xxx`.
            table.entries[0xf0] = SecondLevelPageEntry::page_ptr(vector_table, attr);
        } else {
            // This second-level table is for `0x000xxxxx`.
            // Make a page entry for the virtual address `0x00000xxx`.
            table.entries[0x00] = SecondLevelPageEntry::page_ptr(vector_table, attr);
        }

        table
    };

    const PAGE_TABLE: FirstLevelPageTable = {
        let mut table = FirstLevelPageTable {
            entries: [FirstLevelPageEntry::fault(); 4096],
        };
        let mut occupied = [false; 4096];

        if Self::VECTOR_HIGH {
            table.entries[0xfff] = FirstLevelPageEntry::page_table(&Self::VECTOR_PAGE_TABLE);
            occupied[0xfff] = true;
        } else {
            table.entries[0x000] = FirstLevelPageEntry::page_table(&Self::VECTOR_PAGE_TABLE);
            occupied[0x000] = true;
        }

        // Create section entries based on `MEMORY_MAP`
        let mmap = Self::MEMORY_MAP;
        // FIXME: Work-around for `for` being unsupported in `const fn`
        let mut i = 0;
        while i < mmap.len() {
            let section = &mmap[i];
            let start_i = section.virtual_start / 0x100000;
            let end_i = start_i + section.len / 0x100000;

            // FIXME: Work-around for `for` being unsupported in `const fn`
            let mut k = start_i;
            while k < end_i {
                if occupied[k] {
                    panic!("region overlap; some address ranges are specified more than once");
                }
                table.entries[k] = FirstLevelPageEntry::section(
                    section.physical_start as u32 + ((k - start_i) * 0x100000) as u32,
                    section.attr,
                );
                occupied[k] = true;
                k += 1;
            }

            i += 1;
        }

        table
    };
}

const fn memory_map_maps_va<T: StartupOptions>(va: usize) -> bool {
    let mmap = T::MEMORY_MAP;
    // FIXME: Work-around for `for` being unsupported in `const fn`
    let mut i = 0;
    while i < mmap.len() {
        let section = &mmap[i];
        if va >= section.virtual_start && va <= section.virtual_start + (section.len - 1) {
            return true;
        }
        i += 1;
    }
    false
}

#[repr(align(16384))]
#[derive(Clone, Copy)]
struct FirstLevelPageTable {
    entries: [FirstLevelPageEntry; 0x1000],
}

#[repr(C)]
#[derive(Clone, Copy)]
union FirstLevelPageEntry {
    int: u32,
    ptr: *mut u8,
}

impl FirstLevelPageEntry {
    /// Construct a faulting entry.
    const fn fault() -> Self {
        Self { int: 0 }
    }

    /// Construct a page table entry.
    const fn page_table(table: *const SecondLevelPageTable) -> Self {
        let domain = 0u32;
        let ns = false; // Secure access
        let pxn = false; // not enabling Privilege Execute Never

        Self {
            // Assuming physical address == virtual address for `table`
            ptr: (table as *mut u8).wrapping_add(
                ((domain << 5) | ((ns as u32) << 3) | ((pxn as u32) << 2) | 0b01) as usize,
            ),
        }
    }

    /// Construct a section entry.
    const fn section(pa: u32, attr: MemoryRegionAttributes) -> Self {
        let MemoryRegionAttributes {
            tex,
            c,
            b,
            s,
            ap,
            xn,
        } = attr;
        let domain = 0u32;
        let ns = false; // Secure access
        let ng = false; // global (not Not-Global)
        let pxn = false; // not using Large Physical Address Extension

        assert!(pa & 0xfffff == 0);

        Self {
            int: pa
                | ((ns as u32) << 19)
                | ((ng as u32) << 17)
                | ((s as u32) << 16)
                | ((ap as u32 >> 2) << 15)
                | ((tex as u32) << 12)
                | ((ap as u32 & 0b11) << 10)
                | (domain << 5)
                | ((xn as u32) << 4)
                | ((c as u32) << 3)
                | ((b as u32) << 2)
                | 0b10
                | (pxn as u32),
        }
    }
}

#[repr(align(1024))]
#[derive(Clone, Copy)]
struct SecondLevelPageTable {
    entries: [SecondLevelPageEntry; 256],
}

#[repr(C)]
#[derive(Clone, Copy)]
union SecondLevelPageEntry {
    int: u32,
    ptr: *mut u8,
}

impl SecondLevelPageEntry {
    /// Construct a faulting entry.
    const fn fault() -> Self {
        Self { int: 0 }
    }

    /// Construct a page entry. Assumes physical address == virtual address for
    /// `ptr`. This method essentially creates an alias for `ptr`.
    ///
    /// The 12 LSBs of `ptr` must be zero.
    const fn page_ptr(ptr: *mut u8, attr: MemoryRegionAttributes) -> Self {
        let MemoryRegionAttributes {
            tex,
            c,
            b,
            s,
            ap,
            xn,
        } = attr;
        let ng = false; // global (not Not-Global)

        Self {
            ptr: ptr.wrapping_add(
                (((ng as u32) << 11)
                    | ((s as u32) << 10)
                    | ((ap as u32 >> 2) << 9)
                    | ((tex as u32) << 6)
                    | ((ap as u32 & 0b11) << 4)
                    | ((c as u32) << 3)
                    | ((b as u32) << 2)
                    | 0b10
                    | (xn as u32)) as usize,
            ),
        }
    }
}
