use core::marker::PhantomData;

use crate::{
    kernel::{cfg::CfgBuilder, interrupt, Port},
    utils::ComptimeVec,
};

impl<System: Port> interrupt::InterruptLine<System> {
    /// Construct a `CfgInterruptLineBuilder` to configure an interrupt line in
    /// [a configuration function](crate#static-configuration).
    ///
    /// It's allowed to call this for multiple times for the same interrupt
    /// line. However, each property (such as [`priority`]) cannot be specified
    /// more than once.
    ///
    /// [`priority`]: CfgInterruptLineBuilder::priority
    pub const fn build() -> CfgInterruptLineBuilder<System> {
        CfgInterruptLineBuilder::new()
    }
}

/// Configuration builder type for [`InterruptLine`].
///
/// [`InterruptLine`]: crate::kernel::InterruptLine
pub struct CfgInterruptLineBuilder<System> {
    _phantom: PhantomData<System>,
    line: Option<interrupt::InterruptNum>,
    priority: Option<interrupt::InterruptPriority>,
    enabled: bool,
}

impl<System: Port> CfgInterruptLineBuilder<System> {
    const fn new() -> Self {
        Self {
            _phantom: PhantomData,
            line: None,
            priority: None,
            enabled: false,
        }
    }

    /// [**Required**] Specify the interrupt line to confiigure.
    pub const fn line(self, line: interrupt::InterruptNum) -> Self {
        // FIXME: `Option::is_some` is not `const fn` yet
        if let Some(_) = self.line {
            panic!("`line` is specified twice");
        }
        Self {
            line: Some(line),
            ..self
        }
    }

    /// Specify the initial priority.
    pub const fn priority(self, priority: interrupt::InterruptPriority) -> Self {
        // FIXME: `Option::is_some` is not `const fn` yet
        if let Some(_) = self.priority {
            panic!("`priority` is specified twice");
        }
        Self {
            priority: Some(priority),
            ..self
        }
    }

    /// Specify whether the interrupt linie should be enabled at system startup.
    /// Defaults to `false` (don't enable).
    pub const fn enabled(self, enabled: bool) -> Self {
        Self { enabled, ..self }
    }

    /// Complete the configuration of an interrupt line, returning an
    /// `InterruptLine` object.
    pub const fn finish(self, cfg: &mut CfgBuilder<System>) -> interrupt::InterruptLine<System> {
        let inner = &mut cfg.inner;

        let line_num = if let Some(line) = self.line {
            line
        } else {
            panic!("`line` is not specified");
        };

        // Create a `CfgBuilderInterruptLine` for `line_num` if it doesn't exist
        // yet
        let i = if let Some(i) = vec_position!(inner.interrupt_lines, |il| il.num == line_num) {
            i
        } else {
            inner.interrupt_lines.push(CfgBuilderInterruptLine {
                num: line_num,
                priority: None,
                enabled: false,
            });
            inner.interrupt_lines.len() - 1
        };

        // Update `CfgBuilderInterruptLine` with values from `self`
        let cfg_interrupt_line = inner.interrupt_lines.get_mut(i);

        if let Some(priority) = self.priority {
            // FIXME: `Option::is_some` is not `const fn` yet
            if let Some(_) = cfg_interrupt_line.priority {
                panic!("`priority` is already specified for this interrupt line");
            }
            cfg_interrupt_line.priority = Some(priority);
        }

        if self.enabled {
            cfg_interrupt_line.enabled = true;
        }

        interrupt::InterruptLine::from_num(line_num)
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct CfgBuilderInterruptLine {
    num: interrupt::InterruptNum,
    priority: Option<interrupt::InterruptPriority>,
    enabled: bool,
}

impl CfgBuilderInterruptLine {
    /// Return `true` if the interrupt line is configured with a priority value
    /// that falls within a managed range.
    pub(super) const fn is_initially_managed<System: Port>(&self) -> bool {
        if let Some(priority) = self.priority {
            let range = System::MANAGED_INTERRUPT_PRIORITY_RANGE;
            priority >= range.start && priority < range.end
        } else {
            false
        }
    }
}

impl<System: Port> interrupt::InterruptServiceRoutine<System> {
    /// Construct a `CfgInterruptServiceRoutineBuilder` to register an interrupt
    /// service routine in
    /// [a configuration function](crate#static-configuration).
    pub const fn build() -> CfgInterruptServiceRoutineBuilder<System> {
        CfgInterruptServiceRoutineBuilder::new()
    }
}

/// Configuration builder type for [`InterruptServiceRoutine`].
///
/// [`InterruptServiceRoutine`]: crate::kernel::InterruptServiceRoutine
pub struct CfgInterruptServiceRoutineBuilder<System> {
    _phantom: PhantomData<System>,
    line: Option<interrupt::InterruptNum>,
    start: Option<fn(usize)>,
    param: usize,
    priority: i32,
    unmanaged: bool,
}

impl<System: Port> CfgInterruptServiceRoutineBuilder<System> {
    const fn new() -> Self {
        Self {
            _phantom: PhantomData,
            line: None,
            start: None,
            param: 0,
            priority: 0,
            unmanaged: false,
        }
    }

    /// [**Required**] Specify the entry point.
    pub const fn start(self, start: fn(usize)) -> Self {
        Self {
            start: Some(start),
            ..self
        }
    }

    /// Specify the parameter to `start`. Defaults to `0`.
    pub const fn param(self, param: usize) -> Self {
        Self { param, ..self }
    }

    /// [**Required**] Specify the interrupt line to attach the interrupt
    /// service routine to.
    pub const fn line(self, line: interrupt::InterruptNum) -> Self {
        // FIXME: `Option::is_some` is not `const fn` yet
        if let Some(_) = self.line {
            panic!("`line` is specified twice");
        }
        Self {
            line: Some(line),
            ..self
        }
    }

    /// Specify the priority. Defaults to `0` when unspecified.
    pub const fn priority(self, priority: i32) -> Self {
        Self { priority, ..self }
    }

    /// Indicate that the entry point function is allowed to execute in
    /// [an unmanaged interrupt handler].
    ///
    /// If an interrupt line is not configured with a managed priority value,
    /// configuration will fail unless `unmanaged` is specified for all of its
    /// attached interrupt service routines.
    ///
    /// # Safety
    ///
    /// The behavior of system calls is undefined in an unmanaged interrupt
    /// handler.
    ///
    /// [an unmanaged interrupt handler]: crate#interrupt-handling-framework
    pub const unsafe fn unmanaged(self) -> Self {
        Self {
            unmanaged: true,
            ..self
        }
    }

    /// Complete the registration of an interrupt service routine, returning an
    /// `InterruptServiceRoutine` object.
    pub const fn finish(
        self,
        cfg: &mut CfgBuilder<System>,
    ) -> interrupt::InterruptServiceRoutine<System> {
        let inner = &mut cfg.inner;

        let line_num = if let Some(line) = self.line {
            line
        } else {
            panic!("`line` is not specified");
        };

        inner.isrs.push(CfgBuilderInterruptServiceRoutine {
            line: line_num,
            start: if let Some(x) = self.start {
                x
            } else {
                panic!("`start` is not specified")
            },
            param: self.param,
            priority: self.priority,
            unmanaged: self.unmanaged,
        });

        interrupt::InterruptServiceRoutine::new()
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct CfgBuilderInterruptServiceRoutine {
    line: interrupt::InterruptNum,
    start: fn(usize),
    param: usize,
    priority: i32,
    unmanaged: bool,
}

/// Panic if a non-unmanaged-safe interrupt service routine is attached to an
/// interrupt line that is not known to be managed.
pub(super) const fn panic_if_unmanaged_safety_is_violated<System: Port>(
    interrupt_lines: &ComptimeVec<CfgBuilderInterruptLine>,
    isrs: &ComptimeVec<CfgBuilderInterruptServiceRoutine>,
) {
    // FIXME: Work-around for `for` being unsupported in `const fn`
    let mut i = 0;
    while i < isrs.len() {
        let isr = isrs.get(i);
        i += 1;
        if isr.unmanaged {
            continue;
        }

        // FIXME: Work-around for `Option::is_none` not being `const fn`
        let line_unmanaged = matches!(
            vec_position!(interrupt_lines, |line| line.num == isr.line
                && line.is_initially_managed::<System>()),
            None
        );

        if line_unmanaged {
            panic!(
                "An interrupt service routine that is not marked with `unmanaged` \
                is attached to an interrupt line whose priority value is \
                unspecified or doesn't fall within a managed range."
            );
        }
    }
}
