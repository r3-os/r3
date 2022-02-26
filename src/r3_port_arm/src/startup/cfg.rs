use core::ops::Range;

/// Generate [startup code]. **Requires [`StartupOptions`] and [`EntryPoint`] to
/// be implemented.**
///
/// This macro produces an entry point function whose symbol name is `start`.
/// You should specify it as an entry point in your linker script (the provided
/// linker scripts automatically do this for you).
///
/// [startup code]: crate#startup-code
/// [`EntryPoint`]: crate::EntryPoint
#[macro_export]
macro_rules! use_startup {
    (unsafe $Traits:ty) => {
        #[no_mangle]
        #[naked]
        pub unsafe extern "C" fn start() {
            ::core::arch::asm!(
                "b {}",
                sym $crate::startup::imp::start::<$Traits>,
                options(noreturn),
            );
        }
    };
}

/// The options for [`use_startup!`].
pub trait StartupOptions {
    /// The memory map.
    ///
    /// Note that the kernel code and the startup code don't support relocation,
    /// so you need to make sure they are covered by an identical mapping.
    ///
    /// At least one of `0x0000000` and `0xffff0000` must left unmapped so that
    /// an exception vector table can be placed there.
    ///
    /// # Examples
    ///
    /// ```
    /// use r3_port_arm::MemoryMapSection;
    ///
    /// // Renesas RZ/A1H
    /// const MEMORY_MAP: &'static [MemoryMapSection] = &[
    ///     // On-chip RAM (10MB)
    ///     MemoryMapSection::new(0x2000_0000..0x20a0_0000, 0x2000_0000).with_executable(true),
    ///     // I/O areas
    ///     MemoryMapSection::new(0x3fe0_0000..0x4000_0000, 0x3fe0_0000).as_device_memory(),
    ///     MemoryMapSection::new(0xe800_0000..0xe830_0000, 0xe800_0000).as_device_memory(),
    ///     MemoryMapSection::new(0xfc00_0000..0xfc10_0000, 0xfc00_0000).as_device_memory(),
    ///     MemoryMapSection::new(0xfcf0_0000..0xfd00_0000, 0xfcf0_0000).as_device_memory(),
    /// ];
    /// ```
    const MEMORY_MAP: &'static [MemoryMapSection];
}

#[derive(Debug, Copy, Clone)]
pub struct MemoryMapSection {
    /// The starting physical address. Must be aligned to 1MiB blocks.
    pub(super) physical_start: u64,
    /// The starting virtual address. Must be aligned to 1MiB blocks.
    pub(super) virtual_start: usize,
    /// The length of the section, measured in bytes. Must be aligned to 1MiB
    /// blocks.
    pub(super) len: usize,
    pub(super) attr: MemoryRegionAttributes,
}

impl MemoryMapSection {
    /// Construct a `MemoryMapSection` for normal read/write memory access.
    ///
    ///  - All endpoints must be aligned to 1MiB blocks (`0x???0_0000`).
    ///
    ///  - `virtual_range` must not be empty.
    ///
    ///  - `virtual_range` must be a strict subset of `0..0x1_0000_0000`.
    ///
    ///  - `physical_start` is of type `u64`, but using a large physical address
    ///    (> 4GiB) isn't supported yet.
    ///
    /// The memory section is configured as a read/writable (but not
    /// executable) Normal memory with a Outer and Inner Write-Back,
    /// Write-Allocate attribute.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use r3_port_arm::MemoryMapSection;
    ///
    /// // Map VA `0x2000_0000..0x2800_0000` to PA `0xc000_0000.0xc800_0000`
    /// MemoryMapSection::new(0x2000_0000..0x2800_0000, 0xc000_0000);
    /// ```
    ///
    /// This function panics if an invalid parameter is supplied.
    ///
    /// ```rust,should_panic
    /// # use r3_port_arm::MemoryMapSection;
    /// // Empty range
    /// MemoryMapSection::new(0x2000_0000..0x2000_0000, 0xc000_0000);
    /// ```
    ///
    /// ```rust,should_panic
    /// # use r3_port_arm::MemoryMapSection;
    /// // VA is not in range `0..0x1_0000_0000`
    /// MemoryMapSection::new(0x9000_0000..0x11000_0000, 0xc000_0000);
    /// ```
    pub const fn new(virtual_range: Range<u64>, physical_start: u64) -> Self {
        if (virtual_range.start & 0xfffff) != 0
            || (virtual_range.end & 0xfffff) != 0
            || (physical_start & 0xfffff) != 0
        {
            panic!("all endpoints must be aligned to 1MiB blocks");
        }

        if virtual_range.start >= virtual_range.end {
            panic!("`virtual_range` must not be empty");
        }

        // `<Range as PartialEq>::eq` is not `const fn` yet
        // [ref:range_const_partial_eq]
        if virtual_range.end > 0x1_0000_0000
            || (virtual_range.start == 0 && virtual_range.end == 0x1_0000_0000)
        {
            panic!("`virtual_range` must be a strict subset of `0..0x1_0000_0000`");
        }

        Self {
            physical_start,
            virtual_start: virtual_range.start as usize,
            len: (virtual_range.end - virtual_range.start) as usize,
            attr: MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE,
        }
    }

    /// Modify the memory attribute for a Device memory, returning the modified
    /// `self`.
    pub const fn as_device_memory(self) -> Self {
        Self {
            attr: self.attr.as_device_memory(),
            ..self
        }
    }

    /// Change the sharability, returning the modified `self`.
    pub const fn with_sharable(self, sharable: bool) -> Self {
        Self {
            attr: self.attr.with_sharable(sharable),
            ..self
        }
    }

    /// Change the executability, returning the modified `self`.
    pub const fn with_executable(self, executable: bool) -> Self {
        Self {
            attr: self.attr.with_executable(executable),
            ..self
        }
    }

    /// Change the writability, returning the modified `self`.
    pub const fn with_writable(self, writable: bool) -> Self {
        Self {
            attr: self.attr.with_writable(writable),
            ..self
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub(super) struct MemoryRegionAttributes {
    pub tex: u8,
    pub c: bool,
    pub b: bool,
    pub s: bool,
    pub ap: u8,
    pub xn: bool,
}

impl MemoryRegionAttributes {
    pub(super) const NORMAL_WB_WA_SHARABLE_READ_WRITE: Self = Self {
        tex: 0b001,
        c: true,
        b: true,
        s: true,
        ap: 0b011,
        xn: true,
    };

    const fn as_device_memory(self) -> Self {
        if self.s {
            // Shareable device
            Self {
                tex: 0b000,
                c: false,
                b: true,
                ..self
            }
        } else {
            // Non-shareable device
            Self {
                tex: 0b010,
                c: false,
                b: false,
                ..self
            }
        }
    }

    pub(super) const fn with_sharable(self, sharable: bool) -> Self {
        if self.tex == 0b000 || self.tex == 0b010 {
            if sharable {
                // Shareable device
                Self {
                    tex: 0b000,
                    c: false,
                    b: true,
                    s: sharable,
                    ..self
                }
            } else {
                // Non-shareable device
                Self {
                    tex: 0b010,
                    c: false,
                    b: false,
                    s: sharable,
                    ..self
                }
            }
        } else {
            Self {
                s: sharable,
                ..self
            }
        }
    }

    pub(super) const fn with_executable(self, executable: bool) -> Self {
        Self {
            xn: !executable,
            ..self
        }
    }

    pub(super) const fn with_writable(self, writable: bool) -> Self {
        Self {
            ap: if writable { 0b011 } else { 0b111 },
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_attributes() {
        assert_eq!(
            MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE.as_device_memory(),
            MemoryRegionAttributes {
                tex: 0b000,
                c: false,
                b: true,
                s: true,
                ap: 0b011,
                xn: true,
            },
        );

        // Sharable by default
        assert_eq!(
            MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE
                .as_device_memory()
                .with_sharable(true),
            MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE.as_device_memory(),
        );
        assert_eq!(
            MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE.with_sharable(true),
            MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE,
        );

        // No Execute by default
        assert_eq!(
            MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE.with_executable(false),
            MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE,
        );
        assert_eq!(
            MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE
                .as_device_memory()
                .with_executable(false),
            MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE.as_device_memory(),
        );

        // Writable by default
        assert_eq!(
            MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE.with_writable(true),
            MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE,
        );
        assert_eq!(
            MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE
                .as_device_memory()
                .with_writable(true),
            MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE.as_device_memory(),
        );

        // Non-Sharable Device memory
        assert_eq!(
            MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE
                .as_device_memory()
                .with_sharable(false),
            MemoryRegionAttributes {
                tex: 0b010,
                c: false,
                b: false,
                s: false,
                ap: 0b011,
                xn: true,
            },
        );
        assert_eq!(
            MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE
                .with_sharable(false)
                .as_device_memory(),
            MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE
                .as_device_memory()
                .with_sharable(false),
        );

        // Read-only Non-Sharable Device memory
        assert_eq!(
            MemoryRegionAttributes::NORMAL_WB_WA_SHARABLE_READ_WRITE
                .as_device_memory()
                .with_sharable(false)
                .with_writable(false),
            MemoryRegionAttributes {
                tex: 0b010,
                c: false,
                b: false,
                s: false,
                ap: 0b111,
                xn: true,
            },
        );
    }
}
