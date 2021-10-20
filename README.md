<h1 align="center">
<img src="https://img.shields.io/badge/-𝖱𝟥-222?style=for-the-badge&labelColor=111111" height="100" alt="R3"><br><img src="https://img.shields.io/badge/-𝖱𝖤𝖠𝖫--𝖳𝖨𝖬𝖤%20𝖮𝖯𝖤𝖱𝖠𝖳𝖨𝖭𝖦%20𝖲𝖸𝖲𝖳𝖤𝖬-eee?style=for-the-badge&labelColor=333333" alt="Real-Time Operating System">
</h1>

<p align="center">
<img src="https://img.shields.io/github/workflow/status/yvt/r3/CI/%F0%9F%A6%86?style=for-the-badge"> <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=for-the-badge"> <a href="https://crates.io/crates/r3"><img src="https://img.shields.io/crates/v/r3?style=for-the-badge"></a> <a href="https://yvt.github.io/r3/doc/r3/index.html"><img src="https://yvt.github.io/r3/doc/badge.svg"></a>
</p>

<p align="center">
<a href="https://repl.it/@yvt/R3-Kernel-Hosted-Port#main.rs"><b>Try it on Repl.it</b></a>
</p>

R3 is a proof-of-concept of a static RTOS that utilizes Rust's compile-time function evaluation mechanism for static configuration (creation of kernel objects and memory allocation).

- **All kernel objects are defined statically** for faster boot times, compile-time checking, predictable execution, reduced RAM consumption, no runtime allocation failures, and extra security.
- The kernel and its configurator **don't require an external build tool or a specialized procedural macro**, maintaining transparency.
- The kernel is split into a target-independent portion and a target-specific portion. The target-specific portion (called *a port*) is provided as a separate crate. An application **combines them using the trait system**.
- Leverages Rust's type safety for access control of kernel objects. Safe code can't access an object that it doesn't own.

## Features

- Traditional uniprocessor tickless real-time kernel with preemptive scheduling

- **Tasks** are kernel objects associated with application threads and encapsulate their execution states. Tasks can be activated by application code or automatically at boot time. Tasks are assigned priorities (up to 2¹⁵ levels on a 32-bit target, though the implementation is heavily optimized for a smaller number of priorities), which can be changed at runtime. A task can enable **Priority Boost** to temporarily raise its priority to higher than any other tasks. The number of tasks is only limited by memory available.

- This kernel provides a unified interface to control **interrupt lines** and register **interrupt handlers**. In addition, the Arm M-Profile port supports **unmanaged interrupt lines**, which aren't masked when the kernel is handling a system call.

- This kernel supports common synchronization primitives such as **mutexes**, **semaphores**, and **event groups**. The mutexes can use [the priority ceiling protocol] to avoid unbounded priority inversion and mutual deadlock. Tasks can **park** themselves.

- The kernel timing mechanism drives **software timers** and a **system-global clock** with microsecond precision. The system clock can be rewound or fast-forwarded for drift compensation. The timing algorithm has a logarithmic time complexity and is therefore scalable. The implementation is robust against a large interrupt processing delay.

- The utility library includes safe container types such as **`Mutex`** and **`RecursiveMutex`**, which are built upon low-level synchronization primitives.

- Supports **Arm M-Profile** (all versions shipped so far), **Armv7-A** (no FPU), **RISC-V** as well as **the simulator port** that runs on a host system.

[the priority ceiling protocol]: https://en.wikipedia.org/wiki/Priority_ceiling_protocol

## Example

```rust
#![feature(asm)]
#![feature(const_fn_trait_bound)]
#![feature(const_mut_refs)]
#![feature(const_fn_fn_ptr_basics)]
#![no_std]
#![no_main]

// ----------------------------------------------------------------

// Instantiate the Armv7-M port
use r3_port_arm_m as port;

port::use_port!(unsafe struct System);
port::use_rt!(unsafe System);
port::use_systick_tickful!(unsafe impl PortTimer for System);

impl port::ThreadingOptions for System {}

impl port::SysTickOptions for System {
    // STMF401 default clock configuration
    // SysTick = AHB/8, AHB = HSI (internal 16-MHz RC oscillator)
    const FREQUENCY: u64 = 2_000_000;
}

// ----------------------------------------------------------------

use r3::kernel::{Task, cfg::CfgBuilder};

struct Objects {
    task: Task<System>,
}

// Instantiate the kernel, allocate object IDs
const COTTAGE: Objects = r3::build!(System, configure_app => Objects);

const fn configure_app(b: &mut CfgBuilder<System>) -> Objects {
    System::configure_systick(b);

    Objects {
        task: Task::build()
            .start(task_body)
            .priority(2)
            .active(true)
            .finish(b),
    }
}

fn task_body(_: usize) {
    // ...
}
```

Explore the `examples` directory for example projects.

## Prerequisites

You need a Nightly Rust compiler. This project is heavily reliant on unstable features, so it might or might not work with a newer compiler version. See the file `rust-toolchain` to find out which compiler version this project is currently tested with.

You also need to install Rust's cross-compilation support for your target architecture. If it's not installed, you will see a compile error like this:

```
error[E0463]: can't find crate for `core`
  |
  = note: the `thumbv7m-none-eabi` target may not be installed
```

In this case, you need to run `rustup target add thumbv7m-none-eabi`.
