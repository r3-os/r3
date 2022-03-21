use r3_core::kernel::{
    interrupt::{InterruptHandlerFn, InterruptNum, InterruptPriority},
    raw_cfg::{CfgInterruptLine, InterruptLineDescriptor},
};

use crate::{cfg::CfgBuilder, interrupt, utils::Frozen, KernelTraits};

unsafe impl<Traits: KernelTraits> const CfgInterruptLine for CfgBuilder<Traits> {
    fn interrupt_line_define<Properties: ~const r3_core::bag::Bag>(
        &mut self,
        InterruptLineDescriptor {
            phantom: _,
            line,
            priority,
            start,
            enabled,
        }: InterruptLineDescriptor<Self::System>,
        _properties: Properties,
    ) {
        self.interrupt_lines.push(CfgBuilderInterruptLine {
            line,
            priority,
            start,
            enabled,
        });
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct CfgBuilderInterruptLine {
    line: InterruptNum,
    priority: Option<InterruptPriority>,
    start: Option<InterruptHandlerFn>,
    enabled: bool,
}

impl CfgBuilderInterruptLine {
    pub const fn to_init(&self) -> interrupt::InterruptLineInit {
        interrupt::InterruptLineInit {
            line: self.line,
            priority: self.priority.unwrap_or(0),
            flags: {
                let mut f = 0;
                if self.priority.is_some() {
                    f |= interrupt::InterruptLineInitFlags::SET_PRIORITY.bits();
                }
                if self.enabled {
                    f |= interrupt::InterruptLineInitFlags::ENABLE.bits();
                }
                interrupt::InterruptLineInitFlags::from_bits_truncate(f)
            },
        }
    }
}

#[doc(hidden)]
pub const fn interrupt_handler_table_len(
    interrupt_lines: &[Frozen<CfgBuilderInterruptLine>],
) -> usize {
    let mut num = 0;
    let mut i = 0;
    while i < interrupt_lines.len() {
        let k = interrupt_lines[i].get().line + 1;
        if k > num {
            num = k;
        }
        i += 1;
    }
    num
}

#[doc(hidden)]
pub const fn interrupt_handler_table<const LEN: usize>(
    interrupt_lines: &[Frozen<CfgBuilderInterruptLine>],
) -> [Option<InterruptHandlerFn>; LEN] {
    let mut table = [None; LEN];

    let mut i = 0;
    while i < interrupt_lines.len() {
        if let CfgBuilderInterruptLine {
            line,
            start: Some(start),
            ..
        } = interrupt_lines[i].get()
        {
            assert!(
                table[line].is_none(),
                "an interrupt line's handler is registered twice"
            );
            table[line] = Some(start);
        }

        i += 1;
    }

    table
}

/// A table of combined second-level interrupt handlers.
#[derive(Debug)]
pub struct InterruptHandlerTable {
    #[doc(hidden)]
    pub storage: &'static [Option<InterruptHandlerFn>],
}

impl InterruptHandlerTable {
    /// Get a combined second-level interrupt handler for the specified
    /// interrupt number.
    ///
    /// Returns `None` if no interrupt handlers have been registered for the
    /// specified interrupt number.
    #[inline]
    pub const fn get(&self, line: InterruptNum) -> Option<InterruptHandlerFn> {
        self.storage.get(line).copied().flatten()
    }
}
