use crate::INTERRUPT_SYSTICK;
use r3_kernel::KernelTraits;

/// Used by `use_port!`
#[derive(Clone, Copy)]
pub union InterruptHandler {
    undefined: usize,
    defined: r3_core::kernel::interrupt::InterruptHandlerFn,
}

const NUM_INTERRUPTS: usize = if cfg!(armv6m) { 32 } else { 240 };

pub type InterruptHandlerTable = [InterruptHandler; NUM_INTERRUPTS];

/// Used by `use_port!`
pub const fn make_interrupt_handler_table<Traits: KernelTraits>() -> InterruptHandlerTable {
    let mut table = [InterruptHandler { undefined: 0 }; NUM_INTERRUPTS];

    // `[T]::iter_mut` is unusable in `const fn` [ref:const_slice_iter]
    // `core::array::from_fn` is not `const fn` [ref:const_array_from_fn]
    for i in 0..table.len() {
        table[i] = if let Some(x) = Traits::INTERRUPT_HANDLERS.get(i + 16) {
            InterruptHandler { defined: x }
        } else {
            InterruptHandler { undefined: 0 }
        };
    }

    // Disallow registering in range `0..16` except for SysTick
    for i in 0..16 {
        if i != INTERRUPT_SYSTICK {
            // TODO: This check trips even if no handler is registered at `i`
            #[cfg(any())]
            assert!(
                Traits::INTERRUPT_HANDLERS.get(i).is_none(),
                "registering a handler for a non-interrupt exception is \
                disallowed except for SysTick"
            );
        }
    }

    table
}

#[repr(C, align(4))]
pub struct ExceptionTrampoline {
    _inst: u32,
    _handler: unsafe extern "C" fn(),
}

impl ExceptionTrampoline {
    pub const fn new(target: unsafe extern "C" fn()) -> Self {
        Self {
            _inst: if cfg!(target_feature = "v6t2") {
                // `ldr pc, _handler`
                0xf000f8df
            } else {
                // `ldr r0, _handler; bx r0`
                0x47004800
            },
            _handler: target,
        }
    }
}
