<h1 align="center">
<img src="doc/logo-large-bg.svg" alt="R3 Real-Time Operating System">
</h1>

<p align="center">
<strong>Experimental static component-oriented RTOS for deeply embedded systems</strong>
</p>

<p align="center">
<img src="https://img.shields.io/github/actions/workflow/status/r3-os/r3/ci.yml?branch=%F0%9F%A6%86&style=for-the-badge"> <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=for-the-badge"> <a href="https://crates.io/crates/r3"><img src="https://img.shields.io/crates/v/r3?style=for-the-badge"></a> <a href="https://r3-os.github.io/r3/doc/r3/index.html"><img src="https://r3-os.github.io/r3/doc/badge.svg"></a>
</p>

<p align="center">
<a href="https://replit.com/@yvt/R3-OS-Hosted-Port?v=1#main.rs"><b>Try it on Repl.it</b></a>
</p>

**R3-OS** (or simply **R3**) is an experimental static RTOS that utilizes Rust's compile-time function evaluation mechanism for static configuration (creation of kernel objects and memory allocation) and const traits to decouple kernel interfaces from implementation.

- **All kernel objects are defined statically** for faster boot times, compile-time checking, predictable execution, reduced RAM consumption, no runtime allocation failures, and extra security.
- A kernel and its configurator **don't require an external build tool or a specialized procedural macro**, maintaining transparency and inter-crate composability.
- The kernel API is **not tied to any specific kernel implementations**. Kernels are provided as separate crates, one of which an application chooses and instantiates using the trait system.
- Leverages Rust's type safety for access control of kernel objects. Safe code can't access an object that it doesn't own.

## R3 API

- **Tasks** are the standard way to spawn application threads. They are kernel objects encapsulating the associated threads' execution states and can be activated by application code or automatically at boot time. Tasks have dynamic priorities and can block to relinquish the processor for lower-priority tasks.

- R3 provides a unified interface to control **interrupt lines** and register **interrupt handlers**. Some kernels (e.g., the Arm M-Profile port of the original kernel) support **unmanaged interrupt lines**, which aren't masked when the kernel is handling a system call and thus provide superior real-time performance.

- R3 supports common synchronization primitives such as **mutexes**, **semaphores**, and **event groups**. The mutexes can use [the priority ceiling protocol] to avoid unbounded priority inversion and mutual deadlock. Tasks can **park** themselves.

- The kernel timing mechanism drives **software timers** and a **system-global clock** with microsecond precision. The system clock can be rewound or fast-forwarded for drift compensation.

- **Bindings** are a statically-defined storage with runtime initialization and configuration-time borrow checking. They can be bound to tasks and other objects to provide safe mutable access.

- Procedural kernel configuration facilitates **componentization**. The utility library includes safe container types such as **`Mutex`** and **`RecursiveMutex`**, which are built upon low-level synchronization primitives.

[the priority ceiling protocol]: https://en.wikipedia.org/wiki/Priority_ceiling_protocol

## The Kernel

The R3 original kernel is provided as a separate package [`r3_kernel`][].

- Traditional uniprocessor tickless real-time kernel with preemptive scheduling

- Implements a software-based scheduler supporting a customizable number of task priorities (up to 2¹⁵ levels on a 32-bit target, though the implementation is heavily optimized for a smaller number of priorities) and an unlimited number of tasks.

- Provides a scalable kernel timing mechanism with a logarithmic time complexity. This implementation is robust against a large interrupt processing delay.

- Supports **Arm M-Profile** (all versions shipped so far), **Armv7-A** (no FPU), **RISC-V** as well as **the simulator port** that runs on a host system.

[`r3_kernel`]: https://crates.io/crates/r3_kernel

## Example

```rust
#![feature(const_refs_to_cell)]
#![feature(const_trait_impl)]
#![feature(naked_functions)]
#![feature(const_mut_refs)]
#![feature(asm_const)]
#![no_std]
#![no_main]

// ----------------------------------------------------------------

// Instantiate the Armv7-M port
use r3_port_arm_m as port;

type System = r3_kernel::System<SystemTraits>;
port::use_port!(unsafe struct SystemTraits);
port::use_rt!(unsafe SystemTraits);
port::use_systick_tickful!(unsafe impl PortTimer for SystemTraits);

impl port::ThreadingOptions for SystemTraits {}

impl port::SysTickOptions for SystemTraits {
    // STMF401 default clock configuration
    // SysTick = AHB/8, AHB = HSI (internal 16-MHz RC oscillator)
    const FREQUENCY: u64 = 2_000_000;
}

// ----------------------------------------------------------------

use r3::{bind::bind, kernel::StaticTask, prelude::*};

struct Objects {
    task: StaticTask<System>,
}

// Instantiate the kernel, allocate object IDs
const COTTAGE: Objects = r3_kernel::build!(SystemTraits, configure_app => Objects);

/// Root configuration
const fn configure_app(b: &mut r3_kernel::Cfg<SystemTraits>) -> Objects {
    System::configure_systick(b);

    // Runtime-initialized static storage
    let count = bind((), || 1u32).finish(b);

    Objects {
        // Create a task, giving the ownership of `count`
        task: StaticTask::define()
            .start_with_bind((count.borrow_mut(),), task_body)
            .priority(2)
            .active(true)
            .finish(b),
    }
}

fn task_body(count: &mut u32) {
    // ...
}
```

Explore the `examples` directory for example projects.

## Prerequisites

You need a Nightly Rust compiler. This project is heavily reliant on unstable features, so it might or might not work with a newer compiler version. See the file `rust-toolchain.toml` to find out which compiler version this project is currently tested with.

You also need to install Rust's cross-compilation support for your target architecture. If it's not installed, you will see a compile error like this:

```
error[E0463]: can't find crate for `core`
  |
  = note: the `thumbv7m-none-eabi` target may not be installed
```

In this case, you need to run `rustup target add thumbv7m-none-eabi`.
