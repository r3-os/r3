Simulator for running [`::constance`] on a hosted environment

# Usage

```rust
#![feature(const_loop)]
#![feature(const_fn)]
#![feature(const_if_match)]
#![feature(const_mut_refs)]

// Require `unsafe` even in `unsafe fn` - highly recommended
#![feature(unsafe_block_in_unsafe_fn)]
#![deny(unsafe_op_in_unsafe_fn)]

use constance::kernel::Task;

// Use the simulator port. This macro generates `fn main()`.
constance_port_std::use_port!(unsafe struct System);

const COTTAGE: () = constance::build!(System, configure_app => ());

constance::configure! {
    const fn configure_app(_: &mut CfgBuilder<System>) -> () {
        new! { Task<_>, start = task_body, priority = 1, active = true };
    }
}

fn task_body(_: usize) {
    // The simulator initializes `env_logger` automatically
    log::warn!("yay");
#   // Make sure the program doesn't panic after stalling
#   std::process::exit(0);
}

# // `use_port!` generates `fn main()`, but the test harness cannot detect that
# #[cfg(any())]
# fn main() {}
```

# Interrupts

This port fully supports [the standard interrupt handling framework].

 - The full range of priority values is available. The default priority is `0`.
 - The simulated hardware exposes `1024` (= [`NUM_INTERRUPT_LINES`]) interrupt
   lines.
 - Smaller priority values are prioritized.
 - Negative priority values are considered unmanaged.

[the standard interrupt handling framework]: ::constance#interrupt-handling-framework
[`NUM_INTERRUPT_LINES`]: crate::NUM_INTERRUPT_LINES

## Implementation

Based on the internal user-mode scheduling (UMS) framework, we treat interrupt handlers as UMS worker threads, just like tasks and the dispatcher. The user-mode scheduler manages active interrupt threads and favors them over other kinds of threads. (In contrast, the scheduler doesn't manage tasks - it only knows which task is currently chosen by the operating system.)

**To be implemented:** True asynchronous interrupts aren't supported yet.
