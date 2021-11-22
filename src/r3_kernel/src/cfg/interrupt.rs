use r3::kernel::{
    interrupt::{InterruptHandlerFn, InterruptNum, InterruptPriority},
    raw_cfg::{CfgInterruptLine, InterruptLineDescriptor},
};

use crate::{cfg::CfgBuilder, interrupt, KernelTraits};

unsafe impl<Traits: KernelTraits> const CfgInterruptLine for CfgBuilder<Traits> {
    fn interrupt_line_define<Properties: ~const r3::bag::Bag>(
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
        self.inner.interrupt_lines.push(CfgBuilderInterruptLine {
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
            // FIXME: `Option::unwrap_or` is not `const fn` yet
            priority: if let Some(i) = self.priority { i } else { 0 },
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
