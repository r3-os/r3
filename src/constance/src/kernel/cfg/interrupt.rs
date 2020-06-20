use core::marker::PhantomData;

use crate::{
    kernel::{cfg::CfgBuilder, interrupt, Port},
    utils::{for_times::Nat, ComptimeVec},
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

    pub const fn to_init<System>(&self) -> interrupt::InterruptLineInit<System> {
        interrupt::InterruptLineInit {
            line: interrupt::InterruptLine::from_num(self.num),
            // FIXME: `Option::unwrap_or` is not `const fn` yet
            priority: if let Some(i) = self.priority { i } else { 0 },
            flags: {
                let mut f = 0;
                // FIXME: `Option::is_some` is not `const fn` yet
                if let Some(_) = self.priority {
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

impl<System: Port> interrupt::InterruptHandler<System> {
    /// Construct a `CfgInterruptHandlerBuilder` to register an interrupt
    /// handler in [a configuration function](crate#static-configuration).
    pub const fn build() -> CfgInterruptHandlerBuilder<System> {
        CfgInterruptHandlerBuilder::new()
    }
}

/// Configuration builder type for [`InterruptHandler`].
///
/// [`InterruptHandler`]: crate::kernel::InterruptHandler
pub struct CfgInterruptHandlerBuilder<System> {
    _phantom: PhantomData<System>,
    line: Option<interrupt::InterruptNum>,
    start: Option<fn(usize)>,
    param: usize,
    priority: i32,
    unmanaged: bool,
}

impl<System: Port> CfgInterruptHandlerBuilder<System> {
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
    /// handler to.
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
    ///
    /// When multiple handlers are registered to a single interrupt line, those
    /// with smaller priority values will execute earlier.
    ///
    /// This should not be confused with [an interrupt line's priority].
    ///
    /// [an interrupt line's priority]: CfgInterruptLineBuilder::priority
    pub const fn priority(self, priority: i32) -> Self {
        Self { priority, ..self }
    }

    /// Indicate that the entry point function is unmanaged-safe (designed to
    /// execute as [an unmanaged interrupt handler]).
    ///
    /// If an interrupt line is not configured with an initial priority value
    /// that falls within [a managed range], configuration will fail unless
    /// all of its attached interrupt handlers are marked as
    /// unmanaged-safe.
    ///
    /// [a managed range]: crate::kernel::Port::MANAGED_INTERRUPT_PRIORITY_RANGE
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

    /// Complete the registration of an interrupt handler, returning an
    /// `InterruptHandler` object.
    pub const fn finish(self, cfg: &mut CfgBuilder<System>) -> interrupt::InterruptHandler<System> {
        let inner = &mut cfg.inner;

        let line_num = if let Some(line) = self.line {
            line
        } else {
            panic!("`line` is not specified");
        };

        inner.interrupt_handlers.push(CfgBuilderInterruptHandler {
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

        interrupt::InterruptHandler::new()
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CfgBuilderInterruptHandler {
    line: interrupt::InterruptNum,
    start: fn(usize),
    param: usize,
    priority: i32,
    unmanaged: bool,
}

/// Panic if a non-unmanaged-safe interrupt handler is attached to an
/// interrupt line that is not known to be managed.
pub(super) const fn panic_if_unmanaged_safety_is_violated<System: Port>(
    interrupt_lines: &ComptimeVec<CfgBuilderInterruptLine>,
    interrupt_handlers: &ComptimeVec<CfgBuilderInterruptHandler>,
) {
    // FIXME: Work-around for `for` being unsupported in `const fn`
    let mut i = 0;
    while i < interrupt_handlers.len() {
        let handler = interrupt_handlers.get(i);
        i += 1;
        if handler.unmanaged {
            continue;
        }

        // FIXME: Work-around for `Option::is_none` not being `const fn`
        let line_unmanaged = matches!(
            vec_position!(interrupt_lines, |line| line.num == handler.line
                && line.is_initially_managed::<System>()),
            None
        );

        if line_unmanaged {
            panic!(
                "An interrupt handler that is not marked with `unmanaged` \
                is attached to an interrupt line whose priority value is \
                unspecified or doesn't fall within a managed range."
            );
        }
    }
}

/// Sort interrupt handlers by (interrupt number, priority).
pub(super) const fn sort_handlers(
    interrupt_handlers: &mut ComptimeVec<CfgBuilderInterruptHandler>,
) {
    sort_by!(
        interrupt_handlers.len(),
        |i| interrupt_handlers.get_mut(i),
        |x, y| x.priority < y.priority
    );

    sort_by!(
        interrupt_handlers.len(),
        |i| interrupt_handlers.get_mut(i),
        |x, y| x.line < y.line
    );
}

/// A combined second-level interrupt handler.
///
/// # Safety
///
/// Only meant to be called from a first-level interrupt context. CPU Lock must
/// be inactive.
pub type InterruptHandlerFn = unsafe extern "C" fn();

/// A table of combined second-level interrupt handlers.
///
/// The generic parameter is not a part of a public interface.
pub struct InterruptHandlerTable<T: ?Sized = [Option<InterruptHandlerFn>]> {
    storage: T,
}

impl InterruptHandlerTable {
    /// Get a combined second-level interrupt handler for the specified
    /// interrupt number.
    ///
    /// Returns `None` if no interrupt handlers have been registered for the
    /// specified interrupt number.
    #[inline]
    pub const fn get(&self, line: interrupt::InterruptNum) -> Option<InterruptHandlerFn> {
        // FIXME: `[T]::get` is not `const fn` yet
        if line < self.storage.len() {
            self.storage[line]
        } else {
            None
        }
    }
}

/// The precursor of combined second-level interrupt handlers.
///
/// `MakeProtoCombinedHandlers` generates `ProtoCombinedHandlerFn` for each
/// given (uncombined) interrupt handler. Each `ProtoCombinedHandlerFn` compares
/// the given interrupt number to the one of the handler for which this
/// `ProtoCombinedHandlerFn` was constructed, and if they match, it calls
/// the handler. Then, it proceeds to the next `ProtoCombinedHandlerFn` and this
/// goes on until it hits the end of the list.
///
/// ```rust,ignore
/// fn handler_for_line_3() {
///     proto_combined_handler_0(3, /* ... */);
/// }
///
/// #[inline(always)]
/// fn proto_combined_handler_0(cur_line: interrupt::InterruptNum, /* ... */) {
///     if cur_line == HANDLERS[0].line {
///         (HANDLERS[0].start)();
///     }
///     proto_combined_handler_1(cur_line, /* ... */);
/// }
/// fn proto_combined_handler_1(cur_line: interrupt::InterruptNum, ...) { /* ... */ }
/// fn proto_combined_handler_2(cur_line: interrupt::InterruptNum, ...) { /* ... */ }
/// ```
///
/// `ProtoCombinedHandlerFn` is created from a function that is marked as
/// `#[inline(always)]`. This ensures the aformentioned interrupt number
/// comparison and the calls between `ProtoCombinedHandlerFn`s don't appear in
/// the final binary, assuming some level of compiler optimization is in place.
/// For example, the above code can be optimized as follows:
///
/// ```rust,ignore
/// fn handler_for_line_3() {
///     (HANDLERS[2].start)();
///     (HANDLERS[3].start)();
/// }
/// ```
type ProtoCombinedHandlerFn = fn(interrupt::InterruptNum, bool);

/// The ultimate purpose of this type is to make `PROTO_COMBINED_HANDLERS`
/// (a list of `ProtoCombinedHandlerFn`s) and `FIRST_PROTO_COMBINED_HANDLER`
/// available to `new_interrupt_handler_table`.
struct MakeProtoCombinedHandlers<
    System,
    NumHandlers,
    const HANDLERS: *const CfgBuilderInterruptHandler,
    const NUM_HANDLERS: usize,
>(PhantomData<(System, NumHandlers)>);

trait MakeProtoCombinedHandlersTrait {
    type System: Port;
    type NumHandlers: Nat;
    const HANDLERS: *const CfgBuilderInterruptHandler;
    const NUM_HANDLERS: usize;
    const PROTO_COMBINED_HANDLERS: &'static [ProtoCombinedHandlerFn];
    const FIRST_PROTO_COMBINED_HANDLER: Option<ProtoCombinedHandlerFn>;
}

impl<
        System: Port,
        NumHandlers: Nat,
        const HANDLERS: *const CfgBuilderInterruptHandler,
        const NUM_HANDLERS: usize,
    > MakeProtoCombinedHandlersTrait
    for MakeProtoCombinedHandlers<System, NumHandlers, HANDLERS, NUM_HANDLERS>
{
    type System = System;
    type NumHandlers = NumHandlers;
    const HANDLERS: *const CfgBuilderInterruptHandler = HANDLERS;
    const NUM_HANDLERS: usize = NUM_HANDLERS;
    const PROTO_COMBINED_HANDLERS: &'static [ProtoCombinedHandlerFn] =
        &Self::PROTO_COMBINED_HANDLERS_ARRAY;
    const FIRST_PROTO_COMBINED_HANDLER: Option<ProtoCombinedHandlerFn> = if NUM_HANDLERS > 0 {
        Some(Self::PROTO_COMBINED_HANDLERS_ARRAY[0])
    } else {
        None
    };
}

impl<
        System: Port,
        NumHandlers: Nat,
        const HANDLERS: *const CfgBuilderInterruptHandler,
        const NUM_HANDLERS: usize,
    > MakeProtoCombinedHandlers<System, NumHandlers, HANDLERS, NUM_HANDLERS>
{
    const PROTO_COMBINED_HANDLERS_ARRAY: [ProtoCombinedHandlerFn; NUM_HANDLERS] = const_array_from_fn! {
        fn iter<[T: MakeProtoCombinedHandlersTrait], I: Nat>(ref mut cell: T) -> ProtoCombinedHandlerFn {
            #[inline(always)]
            fn proto_combined_handler<T: MakeProtoCombinedHandlersTrait, I: Nat>(cur_line: interrupt::InterruptNum, mut should_unlock_cpu: bool) {
                // Safety: `I::N < NUM_HANDLERS`
                let handler = unsafe { &*T::HANDLERS.wrapping_add(I::N) };

                if cur_line == handler.line {
                    if should_unlock_cpu {
                        // Relinquish CPU Lock before calling the next handler
                        if T::System::is_cpu_lock_active() {
                            // Safety: CPU Lock active, we have the ownership
                            // of the current CPU Lock (because a previously
                            // called handler left it active)
                            unsafe { T::System::leave_cpu_lock() };
                        }
                    }

                    (handler.start)(handler.param);

                    should_unlock_cpu = true;
                }

                // Call the next proto combined handler
                let i = I::N + 1;
                if i < T::NUM_HANDLERS {
                    T::PROTO_COMBINED_HANDLERS[i](cur_line, should_unlock_cpu);
                }
            }
            proto_combined_handler::<T, I>
        }

        // `Self: MakeProtoCombinedHandlersTrait` is used as the context type
        // for the iteration
        (0..NUM_HANDLERS).map(|i| iter::<[Self], i>(Self(PhantomData))).collect::<[_; NumHandlers]>()
    };
}

// FIXME: ICE results if this has type `&'static [_]`.
//        Pointer generic parameters entail raw pointer comparison
//        (<https://github.com/rust-lang/rust/issues/53020>), which has
//        unclear aspects and thus is unstable at this point.
// FIXME: ↑ This was meant to be inserted before `const HANDLERS: ...`, but when
//        I did that, rustfmt tried to destroy the code
//        <https://github.com/rust-lang/rustfmt/issues/4263>

/// Construct `InterruptHandlerTable`. Only meant to be used by `build!`
///
/// # Safety
///
/// `std::slice::from_raw_parts(HANDLERS, NUM_HANDLERS)` must be a valid
/// reference.
#[doc(hidden)]
pub const unsafe fn new_interrupt_handler_table<
    System: Port,
    NumLines: Nat,
    NumHandlers: Nat,
    const HANDLERS: *const CfgBuilderInterruptHandler,
    const NUM_HANDLERS: usize,
    const NUM_LINES: usize,
>() -> InterruptHandlerTable<[Option<InterruptHandlerFn>; NUM_LINES]> {
    // Check generic parameters

    // Actually, these equality is automatically checked by
    // `const_array_from_fn!`, but do the check here as well to clarify
    // this function's precondition
    // FIXME: `assert!` not supported in a const context yet
    if NumLines::N != NUM_LINES {
        panic!("`NumLines::N != NUM_LINES`");
    }
    if NumHandlers::N != NUM_HANDLERS {
        panic!("`NumHandlers::N != NUM_HANDLERS`");
    }

    // FIXME: Work-around for `for` being unsupported in `const fn`
    let mut i = 0;
    while i < NUM_HANDLERS {
        // Safety: `i < NUM_HANDLERS`. MIRI (the compile-time interpreter)
        // actually can catch unsafe pointer references.
        let handler = unsafe { &*HANDLERS.wrapping_add(i) };
        if handler.line >= NUM_LINES {
            panic!("`handler.line >= NUM_LINES`");
        }
        i += 1;
    }

    let storage = const_array_from_fn! {
        fn iter<[T: MakeProtoCombinedHandlersTrait], I: Nat>(ref mut cell: T) -> Option<InterruptHandlerFn> {
            unsafe extern fn handler<T: MakeProtoCombinedHandlersTrait, I: Nat>() {
                if let Some(proto_combined_handler) = T::FIRST_PROTO_COMBINED_HANDLER {
                    proto_combined_handler(I::N, false);
                }
            }

            if let Some(_) = T::FIRST_PROTO_COMBINED_HANDLER {
                Some(handler::<T, I> as InterruptHandlerFn)
            } else {
                None
            }
        }

        (0..NUM_LINES).map(|i| iter::<[MakeProtoCombinedHandlers<
            System,
            NumHandlers,
            HANDLERS,
            NUM_HANDLERS
        >], i>(MakeProtoCombinedHandlers(PhantomData))).collect::<[_; NumLines]>()
    };

    InterruptHandlerTable { storage }
}

#[doc(hidden)]
pub const fn num_required_interrupt_line_slots(handlers: &[CfgBuilderInterruptHandler]) -> usize {
    // FIXME: Work-around for `for` being unsupported in `const fn`
    let mut i = 0;
    let mut out = 0;
    while i < handlers.len() {
        if handlers[i].line + 1 > out {
            out = handlers[i].line + 1;
        }
        i += 1;
    }
    out
}
