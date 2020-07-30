<h1 align="center">
<img src="https://img.shields.io/badge/-𝖢𝖮𝖭𝖲𝖳𝖠𝖭𝖢𝖤-222?style=for-the-badge&labelColor=111111" width="40%" height="auto" alt="Constance"><img src="https://img.shields.io/badge/-𝖱𝖤𝖠𝖫--𝖳𝖨𝖬𝖤%20𝖮𝖯𝖤𝖱𝖠𝖳𝖨𝖭𝖦%20𝖲𝖸𝖲𝖳𝖤𝖬-666?style=for-the-badge&labelColor=333333" width="50%" height="auto" alt="Real-Time Operating System">
</h1>

<p align="center">
<img src="https://img.shields.io/github/workflow/status/yvt/Constance/CI/%F0%9F%A6%86?style=for-the-badge"> <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=for-the-badge"> <img src="https://img.shields.io/badge/crates.io-not%20yet-red?style=for-the-badge"> <img src="https://img.shields.io/badge/docs.rs-I%20wish-red?style=for-the-badge">
</p>

<p align="center">
<a href="https://repl.it/@yvt/Constance-Hosted-Port#main.rs"><b>Try it on Repl.it</b></a>
</p>

Constance is a proof-of-concept of a static RTOS that utilizes Rust's compile-time function evaluation mechanism for static configuration (creation of kernel objects and memory allocation).

- **All kernel objects are defined statically** for faster boot times, compile-time checking, predictable execution, reduced RAM consumption, no runtime allocation failures, and extra security.
- The kernel and its configurator **don't require an external build tool or a specialized procedural macro**, maintaining transparency.
- The kernel is written in a target-independent way. The target-specific portion (called *a port*) is provided as a separate crate, which an application chooses and **combines with the kernel using the trait system**.
- Leverages Rust's type safety for access control of kernel objects. Safe code can't access an object that it doesn't own.

## Implementation Status

|       Core       |     Library     |        Ports       |
| :--------------- | :-------------- | :----------------- |
| ☑︎ Tasks          | ☐ `Mutex`       | ☑︎ `std` (Hosted)   |
| ☑︎ Hunks          | ☐ `RwLock`      | ☑︎ Armv7-M (no FPU) |
| ☑︎ Wait Objects   | ☐ `Once`        | ☑︎ Armv6-M          |
| ☑︎ Timeouts       | ☐ Logger        |                    |
| ☐ Semaphores     | ☐ C API         |                    |
| ☑︎ Event Groups   |                 |                    |
| ☐ Mutexes        | **Tools**       | **Boards**         |
| ☑︎ Timers         | ☑︎ Test Harness  | ☑︎ Hosted           |
| ☑︎ Interrupts     | ☑︎ Test Suite    | ☑︎ F401RE           |
| ☑︎ Startup Hooks  | ☑︎ Configurator  |                    |
| ☐ CPU Exceptions |                 |                    |
| ☐ Panicking      |                 |                    |

## Example

```rust
#![feature(const_fn)]
#![feature(const_mut_refs)]
#![no_std]
#![no_main]

// ----------------------------------------------------------------

// Instantiate the Armv7-M port
use constance_port_arm_m as port;

port::use_port!(unsafe struct System);
port::use_systick_tickful!(unsafe impl PortTimer for System);

impl port::ThreadingOptions for System {}

impl port::SysTickOptions for System {
    // STMF401 default clock configuration
    // SysTick = AHB/8, AHB = HSI (internal 16-MHz RC oscillator)
    const FREQUENCY: u64 = 2_000_000;
}

// ----------------------------------------------------------------

use constance::kernel::{Task, cfg::CfgBuilder};

struct Objects {
    task: Task<System>,
}

// Instantiate the kernel, allocate object IDs
const COTTAGE: Objects = constance::build!(System, configure_app => Objects);

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

## Getting Started

Start the `basic` example application using the simulator port:

```shell
cargo run -p constance_example_basic
```

Start the `basic` example application using [the NUCLEO-F401RE board](https://www.st.com/en/evaluation-tools/nucleo-f401re.html) and [`cargo-embed`](https://crates.io/crates/cargo-embed) for flashing:

```shell
cd examples/basic_nucleo_f401re
cargo embed --release
```

## Prerequisites

You need a Nightly Rust compiler. This project is heavily reliant on unstable features, so it might or might not work with a newer compiler version. See the file `rust-toolchain` to find out which compiler version this project is currently tested with.

You also need to install Rust's cross-compilation support for your target architecture. If it's not installed, you will see a compile error like this:

```
error[E0463]: can't find crate for `core`
  |
  = note: the `thumbv7m-none-eabi` target may not be installed
```

In this case, you need to run `rustup target add thumbv7m-none-eabi`.

## For Developers

### Prerequisites

 - [rustup], which will automatically install the version of Nightly Rust compiler specified by `rust-toolchain`
 - [QEMU](https://www.qemu.org/) 4.2 or later to test the Arm-M port.
 - libusb 1.x to test the Arm-M port.

[rustup]: https://rustup.rs/

[Nix] users can use the provided `shell.nix` file to install all required software.

[Nix]: https://nixos.org/nix/

### How to Run Tests

 - Hosted platform and target-independent tests: `cargo test --all`
 - The Armv7-M port and NUCLEO-F401RE: `cargo run -p constance_port_arm_m_test_runner -- -t nucleo_f401re`
 - The Armv7-M port and Arm MPS2+ AN385 (QEMU emulation): `cargo run -p constance_port_arm_m_test_runner -- -t qemu_mps2_an385`
 - The Armv6-M port and Arm MPS2+ AN385 (QEMU emulation): `cargo run -p constance_port_arm_m_test_runner -- -t qemu_mps2_an385_v6m`
