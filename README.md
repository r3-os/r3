# <img src="https://img.shields.io/badge/C o n s t a n c e-𝖱𝖤𝖠𝖫̵%20𝖳𝖨𝖬𝖤%20𝖮𝖯𝖤𝖱𝖠𝖳𝖨𝖭𝖦%20𝖲𝖸𝖲𝖳𝖤𝖬-666?style=for-the-badge&labelColor=333333" width="90%" height="auto" alt="Constance Real-Time Operating System">

<img src="https://img.shields.io/github/workflow/status/yvt/Constance/CI/%F0%9F%A6%86?style=for-the-badge"> <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=for-the-badge"> <img src="https://img.shields.io/badge/crates.io-not%20yet-red?style=for-the-badge"> <img src="https://img.shields.io/badge/docs.rs-I%20wish-red?style=for-the-badge">

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
| ☑︎ Wait Objects   | ☐ `Once`        |                    |
| ☑︎ Timeouts       | ☐ Logger        |                    |
| ☐ Semaphores     | ☐ C API         |                    |
| ☑︎ Event Groups   |                 |                    |
| ☐ Mutexes        | **Tools**       | **Boards**         |
| ☐ Timers         | ☑︎ Test Harness  | ☑︎ Hosted           |
| ☐ Alarms         | ☑︎ Test Suite    | ☑︎ F401RE           |
| ☑︎ Interrupts     | ☑︎ Configurator  |                    |
| ☑︎ Startup Hooks  |                 |                    |
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

## For Developers

### How to Run Tests

 - Hosted platform and target-independent tests: `cargo test --all`
 - The Arm-M port and NUCLEO-F401RE: `cargo run -p constance_port_arm_m_test_runner -- -t nucleo_f401re`
 - The Arm-M port and Arm MPS2+ AN385 (QEMU emulation): `cargo run -p constance_port_arm_m_test_runner -- -t qemu_mps2_an385`
