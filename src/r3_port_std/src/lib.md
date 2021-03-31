Simulator for running [`::r3`] on a hosted environment

# Usage

```rust
#![feature(const_fn)]
#![feature(const_mut_refs)]
#![feature(const_fn_fn_ptr_basics)]

// Require `unsafe` even in `unsafe fn` - highly recommended
#![deny(unsafe_op_in_unsafe_fn)]

use r3::kernel::{Task, cfg::CfgBuilder};

// Use the simulator port. This macro generates `fn main()`.
r3_port_std::use_port!(unsafe struct System);

const COTTAGE: () = r3::build!(System, configure_app => ());

const fn configure_app(b: &mut CfgBuilder<System>) -> () {
    Task::build()
        .start(task_body)
        .priority(1)
        .active(true)
        .finish(b);
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

[the standard interrupt handling framework]: ::r3#interrupt-handling-framework
[`NUM_INTERRUPT_LINES`]: crate::NUM_INTERRUPT_LINES

## Implementation

Based on the internal user-mode scheduling (UMS) framework, we treat interrupt handlers as UMS worker threads, just like tasks. The user-mode scheduler manages active interrupt threads and favors them over other kinds of threads. (In contrast, the scheduler doesn't manage tasks - it only knows which task is currently chosen by the operating system.)

The interrupt line [`INTERRUPT_LINE_DISPATCH`] is reserved for the dispatcher.

[`INTERRUPT_LINE_DISPATCH`]: crate::INTERRUPT_LINE_DISPATCH

# Preemption and Host Environment

The user-mode scheduling scheme may interact poorly with other components or the host operating system. Preemption is implemented by signals on POSIX platforms and can cause system calls to fail with an error code that `libstd` is not prepared to deal with. Also, sharing an external resource between threads is prone to a deadlock. Here's an example: Suppose an application uses an allocator whose internal structure is protected by a host mutex. Task A acquires a lock, but then gets preempted by task B, which also attempts to acquire a lock. The guest operating system is unaware of the existence of such resources and keeps scheduling task B (not knowing that completing task A would unblock task B), leading to a deadlock.

**This means that even using the default (system) global allocator inside a guest environment can cause a deadlock.**

There are several ways to tackle these problems:

 - Lock the scheduler structure by calling [`lock_scheduler`].

   **Con:** You need to be careful not to call any guest operating services while the lock is being held.

 - Activate [CPU Lock] while accessing external resources.

   **Con:** CPU Lock doesn't affect unmanaged interrupt handlers. Many guest operating services are unavailable while CPU Lock is being held.

 - Activate [Priority Boost] while accessing external resources.

   **Con:** Priority Boost doesn't affect and can't be used in interrupt handlers.

 - Create a mutex using the guest operating system's feature and use it to ensure only one task can access a particular external resource at a time.

   **Con:** Interrupt handlers can't perform a blocking operation. Interrupts can still preempt host system calls.

 - Create an asynchronous RPC channel.

   **Con:** Complicated and requires allocation of system-global resources such as an interrupt line for inbound signaling.

[`lock_scheduler`]: crate::lock_scheduler
[CPU Lock]: r3::kernel::Kernel::acquire_cpu_lock
[Priority Boost]: r3::kernel::Kernel::boost_priority
