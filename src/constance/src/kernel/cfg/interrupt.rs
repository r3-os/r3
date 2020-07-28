use core::marker::PhantomData;

use crate::{
    kernel::{cfg::CfgBuilder, interrupt, Port, PortThreading},
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
#[must_use = "must call `finish()` to complete registration"]
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
        assert!(self.line.is_none(), "`line` is specified twice");
        Self {
            line: Some(line),
            ..self
        }
    }

    /// Specify the initial priority.
    pub const fn priority(self, priority: interrupt::InterruptPriority) -> Self {
        assert!(self.priority.is_none(), "`priority` is specified twice");
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

        // FIXME: Work-around for `Option::expect` being not `const fn`
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
            assert!(
                cfg_interrupt_line.priority.is_none(),
                "`priority` is already specified for this interrupt line"
            );
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
        assert!(self.line.is_none(), "`line` is specified twice");
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
    /// [a managed range]: crate::kernel::PortInterrupts::MANAGED_INTERRUPT_PRIORITY_RANGE
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

        // FIXME: Work-around for `Option::expect` being not `const fn`
        let line_num = if let Some(line) = self.line {
            line
        } else {
            panic!("`line` is not specified");
        };

        let order = inner.interrupt_handlers.len();
        inner.interrupt_handlers.push(CfgBuilderInterruptHandler {
            line: line_num,
            // FIXME: Work-around for `Option::expect` being not `const fn`
            start: if let Some(x) = self.start {
                x
            } else {
                panic!("`start` is not specified")
            },
            param: self.param,
            priority: self.priority,
            unmanaged: self.unmanaged,
            order,
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
    /// The registration order.
    order: usize,
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

        let managed_line_i = vec_position!(interrupt_lines, |line| line.num == handler.line
            && line.is_initially_managed::<System>());
        let is_line_managed = managed_line_i.is_some();

        assert!(
            is_line_managed,
            "An interrupt handler that is not marked with `unmanaged` \
            is attached to an interrupt line whose priority value is \
            unspecified or doesn't fall within a managed range."
        );
    }
}

/// Sort interrupt handlers by (interrupt number, priority, order).
pub(super) const fn sort_handlers(
    interrupt_handlers: &mut ComptimeVec<CfgBuilderInterruptHandler>,
) {
    sort_unstable_by!(
        interrupt_handlers.len(),
        |i| interrupt_handlers.get_mut(i),
        |x, y| if x.line != y.line {
            x.line < y.line
        } else if x.priority != y.priority {
            x.priority < y.priority
        } else {
            x.order < y.order
        }
    );
}

/// A combined second-level interrupt handler.
///
/// # Safety
///
/// Only meant to be called from a first-level interrupt handler. CPU Lock must
/// be inactive.
pub type InterruptHandlerFn = unsafe extern "C" fn();

/// A table of combined second-level interrupt handlers.
///
/// The generic parameter is not a part of the public interface.
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
/// `MakeCombinedHandlers` generates `ProtoCombinedHandlerFn` for each
/// given (uncombined) interrupt handler. Each `ProtoCombinedHandlerFn` calls
/// the handler. Then, it proceeds to the next `ProtoCombinedHandlerFn` and this
/// goes on until it calls the last handler of the current interrupt number.
///
/// ```rust,ignore
/// // `MakeCombinedHandlersTrait::PROTO_COMBINED_HANDLERS`
/// #[inline(always)]
/// fn proto_combined_handler_0(/* ... */) {
///     (HANDLERS[0].start)();
///     if 1 >= HANDLERS.len() || HANDLERS[0].line != HANDLERS[1].line {
///         return;
///     }
///     proto_combined_handler_1(cur_line, /* ... */);
/// }
/// fn proto_combined_handler_1(...) { /* ... */ }
/// fn proto_combined_handler_2(...) { /* ... */ }
/// ```
///
/// `ProtoCombinedHandlerFn` is created from a function that is marked as
/// `#[inline(always)]`. This ensures the chained calls between
/// `ProtoCombinedHandlerFn`s don't appear in the final binary, assuming some
/// level of compiler optimization is in place.
///
/// The final product of `MakeCombinedHandlers` is a combined second-level
/// interrupt handler for each interrupt number, which calls the first
/// `ProtoCombinedHandlerFn` of that interrupt number.
///
/// ```rust,ignore
/// // `MakeCombinedHandlersTrait::COMBINED_HANDLERS`
/// extern "C" fn combined_handler_for_line_3() {
///     proto_combined_handler_2(/* ... */);
/// }
/// ```
///
/// Because of inlining, the above code is optimized as follows:
///
/// ```rust,ignore
/// extern "C" fn combined_handler_for_line_3() {
///     (HANDLERS[2].start)();
///     (HANDLERS[3].start)();
/// }
/// ```
type ProtoCombinedHandlerFn = fn();

// FIXME: Passing `&'static [_]` as a const generic parameter causes ICE:
//        <https://github.com/rust-lang/rust/issues/73727>
//       `CfgBuilderInterruptHandlerList` is a work-around for this issue.
/// A static list of [`CfgBuilderInterruptHandler`]s.
#[doc(hidden)]
pub trait CfgBuilderInterruptHandlerList {
    /// `U<Self::NUM_HANDLERS>`
    type NumHandlers: Nat;
    const HANDLERS: &'static [CfgBuilderInterruptHandler];
}

/// The ultimate purpose of this type is to make `COMBINED_HANDLERS`
/// (a list of `InterruptHandlerFn`s) available to
/// `new_interrupt_handler_table`.
struct MakeCombinedHandlers<System, Handlers, const NUM_HANDLERS: usize>(
    PhantomData<(System, Handlers)>,
);

trait MakeCombinedHandlersTrait {
    type System: Port;
    type NumHandlers: Nat;
    const HANDLERS: &'static [CfgBuilderInterruptHandler];
    const NUM_HANDLERS: usize;
    const PROTO_COMBINED_HANDLERS: &'static [ProtoCombinedHandlerFn];
    const COMBINED_HANDLERS: &'static [Option<InterruptHandlerFn>];
}

impl<System: Port, Handlers: CfgBuilderInterruptHandlerList, const NUM_HANDLERS: usize>
    MakeCombinedHandlersTrait for MakeCombinedHandlers<System, Handlers, NUM_HANDLERS>
{
    type System = System;
    type NumHandlers = Handlers::NumHandlers;
    const HANDLERS: &'static [CfgBuilderInterruptHandler] = Handlers::HANDLERS;
    const NUM_HANDLERS: usize = NUM_HANDLERS;
    const PROTO_COMBINED_HANDLERS: &'static [ProtoCombinedHandlerFn] =
        &Self::PROTO_COMBINED_HANDLERS_ARRAY;
    const COMBINED_HANDLERS: &'static [Option<InterruptHandlerFn>] = &Self::COMBINED_HANDLERS_ARRAY;
}

impl<System: Port, Handlers: CfgBuilderInterruptHandlerList, const NUM_HANDLERS: usize>
    MakeCombinedHandlers<System, Handlers, NUM_HANDLERS>
{
    const PROTO_COMBINED_HANDLERS_ARRAY: [ProtoCombinedHandlerFn; NUM_HANDLERS] =
        Self::proto_combined_handlers_array();

    const fn proto_combined_handlers_array() -> [ProtoCombinedHandlerFn; NUM_HANDLERS] {
        // FIXME: Unable to do this inside a `const` item because of
        //        <https://github.com/rust-lang/rust/pull/72934>
        const_array_from_fn! {
            fn iter<[T: MakeCombinedHandlersTrait], I: Nat>(ref mut cell: T) -> ProtoCombinedHandlerFn {
                #[inline(always)]
                fn proto_combined_handler<T: MakeCombinedHandlersTrait, I: Nat>() {
                    let handler = T::HANDLERS[I::N];

                    (handler.start)(handler.param);

                    let next_i = I::N + 1;
                    if next_i >= T::NUM_HANDLERS || T::HANDLERS[next_i].line != handler.line {
                        return;
                    }

                    // Relinquish CPU Lock before calling the next handler
                    if T::System::is_cpu_lock_active() {
                        // Safety: CPU Lock active, we have the ownership
                        // of the current CPU Lock (because a previously
                        // called handler left it active)
                        unsafe { T::System::leave_cpu_lock() };
                    }

                    // Call the next proto combined handler
                    T::PROTO_COMBINED_HANDLERS[next_i]();
                }
                proto_combined_handler::<T, I>
            }

            // `Self: MakeCombinedHandlersTrait` is used as the context type
            // for the iteration
            (0..NUM_HANDLERS).map(|i| iter::<[Self], i>(Self(PhantomData))).collect::<[_; Handlers::NumHandlers]>()
        }
    }

    const COMBINED_HANDLERS_ARRAY: [Option<InterruptHandlerFn>; NUM_HANDLERS] =
        Self::combined_handlers_array();

    const fn combined_handlers_array() -> [Option<InterruptHandlerFn>; NUM_HANDLERS] {
        // FIXME: Unable to do this inside a `const` item because of
        //        <https://github.com/rust-lang/rust/pull/72934>
        const_array_from_fn! {
            fn iter<[T: MakeCombinedHandlersTrait], I: Nat>(ref mut cell: T) -> Option<InterruptHandlerFn> {
                extern "C" fn combined_handler<T: MakeCombinedHandlersTrait, I: Nat>() {
                    T::PROTO_COMBINED_HANDLERS[I::N]();
                }

                let handler = T::HANDLERS[I::N];
                let is_first_handler_of_line = if I::N == 0 {
                    true
                } else {
                    T::HANDLERS[I::N - 1].line != handler.line
                };

                if is_first_handler_of_line {
                    Some(combined_handler::<T, I> as InterruptHandlerFn)
                } else {
                    None
                }
            }

            // `Self: MakeCombinedHandlersTrait` is used as the context type
            // for the iteration
            (0..NUM_HANDLERS).map(|i| iter::<[Self], i>(Self(PhantomData))).collect::<[_; Handlers::NumHandlers]>()
        }
    }
}

/// Construct `InterruptHandlerTable`. Only meant to be used by `build!`
#[doc(hidden)]
pub const unsafe fn new_interrupt_handler_table<
    System: Port,
    NumLines: Nat,
    Handlers: CfgBuilderInterruptHandlerList,
    const NUM_LINES: usize,
    const NUM_HANDLERS: usize,
>() -> InterruptHandlerTable<[Option<InterruptHandlerFn>; NUM_LINES]> {
    // Check generic parameters

    // Actually, these equality is automatically checked by
    // `const_array_from_fn!`, but do the check here as well to clarify
    // this function's precondition
    // FIXME: `assert_eq!` not supported in a const context yet
    assert!(NumLines::N == NUM_LINES);
    assert!(Handlers::NumHandlers::N == NUM_HANDLERS);

    // FIXME: Work-around for `for` being unsupported in `const fn`
    let mut i = 0;
    while i < NUM_HANDLERS {
        let handler = Handlers::HANDLERS[i];
        assert!(handler.line < NUM_LINES);
        i += 1;
    }

    let storage = const_array_from_fn! {
        fn iter<[T: MakeCombinedHandlersTrait], I: Nat>(ref mut cell: T) -> Option<InterruptHandlerFn> {
            // The interrupt line
            let line = I::N;

            // Find the first handler for the line. The elements of
            // `COMBINED_HANDLERS` are only set for the first handler of each
            // line.
            let i = lower_bound!(T::NUM_HANDLERS, |i| T::HANDLERS[i].line < line);

            if i >= T::NUM_HANDLERS || T::HANDLERS[i].line != line {
                // The interrupt line does not have an associated handler
                None
            } else {
                // Return the combined handler
                let handler = T::COMBINED_HANDLERS[i];
                assert!(handler.is_some());
                handler
            }
        }

        (0..NUM_LINES).map(|i| iter::<[MakeCombinedHandlers<
            System,
            Handlers,
            NUM_HANDLERS,
        >], i>(MakeCombinedHandlers(PhantomData))).collect::<[_; NumLines]>()
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
