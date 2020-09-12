<h1 align="center">
<img src="https://img.shields.io/badge/-𝖢𝖮𝖭𝖲𝖳𝖠𝖭𝖢𝖤-222?style=for-the-badge&labelColor=111111" width="40%" height="auto" alt="Constance"><img src="https://img.shields.io/badge/-𝖱𝖤𝖠𝖫--𝖳𝖨𝖬𝖤%20𝖮𝖯𝖤𝖱𝖠𝖳𝖨𝖭𝖦%20𝖲𝖸𝖲𝖳𝖤𝖬-666?style=for-the-badge&labelColor=333333" width="50%" height="auto" alt="Real-Time Operating System">
</h1>

<p align="center">
<img src="https://img.shields.io/github/workflow/status/yvt/Constance/CI/%F0%9F%A6%86?style=for-the-badge"> <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=for-the-badge"> <img src="https://img.shields.io/badge/crates.io-not%20yet-red?style=for-the-badge"> <a href="https://yvt.github.io/Constance/doc/constance/index.html"><img src="https://yvt.github.io/Constance/doc/badge.svg"></a>
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

| Category               | Status       |
| ---------------------- | ------------ |
| System Topology        | ![Uniprocessor: Supported] ![Homogeneous Multiprocessor: Under Consideration] ![Heterogeneous Multiprocessor: Not Considering] |
| Kernel Core            | ![Tasks: Supported] ![Hunks: Supported] ![Wait Objects: Supported] ![Timeouts: Supported] ![Timers: Supported] ![Interrupts: Supported] ![Startup Hooks: Supported] ![CPU Exceptions: Under Consideration] ![Panicking: Under Consideration] |
| Kernel Synchronization | ![Semaphores: Under Consideration] ![Event Groups: Supported] ![Mutexes: Under Consideration] |
| Library                | ![Mutex: Under Consideration] ![RwLock: Under Consideration] ![Once: Under Consideration] ![C API: Under Consideration] |
| Ports (Simulator)      | ![POSIX: Supported] ![Windows: Under Consideration] |
| Ports (Arm M-Profile)  | ![Armv8-M Mainline (no CMSE): Supported] ![Armv8-M Baseline (no CMSE): Supported] ![Armv7-M: Supported] ![Armv6-M: Supported] |
| Ports (Arm A-Profile)  | ![Armv7-A (no FPU): Supported] |
| Ports (RISC-V)         | ![RV32IMACFD: Supported] ![RV64IMACFD: Supported] |

[Uniprocessor: Supported]: https://img.shields.io/badge/Uniprocessor-Supported-success?style=flat-square
[Homogeneous Multiprocessor: Under Consideration]: https://img.shields.io/badge/Homogeneous%20Multiprocessor-Under%20Consideration-cc7070?style=flat-square
[Heterogeneous Multiprocessor: Not Considering]: https://img.shields.io/badge/Heterogeneous%20Multiprocessor-Not%20Considering-inactive?style=flat-square

[Tasks: Supported]: https://img.shields.io/badge/Tasks-Supported-success?style=flat-square
[Hunks: Supported]: https://img.shields.io/badge/Hunks-Supported-success?style=flat-square
[Wait Objects: Supported]: https://img.shields.io/badge/Wait%20Objects-Supported-success?style=flat-square
[Timeouts: Supported]: https://img.shields.io/badge/Timeouts-Supported-success?style=flat-square
[Semaphores: Under Consideration]: https://img.shields.io/badge/Semaphores-Under%20Consideration-cc7070?style=flat-square
[Event Groups: Supported]: https://img.shields.io/badge/Event%20Groups-Supported-success?style=flat-square
[Mutexes: Under Consideration]: https://img.shields.io/badge/Mutexes-Under%20Consideration-cc7070?style=flat-square
[Timers: Supported]: https://img.shields.io/badge/Timers-Supported-success?style=flat-square
[Interrupts: Supported]: https://img.shields.io/badge/Interrupts-Supported-success?style=flat-square
[Startup Hooks: Supported]: https://img.shields.io/badge/Startup%20Hooks-Supported-success?style=flat-square
[CPU Exceptions: Under Consideration]: https://img.shields.io/badge/CPU%20Exceptions-Under%20Consideration-cc7070?style=flat-square
[Panicking: Under Consideration]: https://img.shields.io/badge/Panicking-Under%20Consideration-cc7070?style=flat-square

[Mutex: Under Consideration]: https://img.shields.io/badge/Mutex-Under%20Consideration-cc7070?style=flat-square
[RwLock: Under Consideration]: https://img.shields.io/badge/RwLock-Under%20Consideration-cc7070?style=flat-square
[Once: Under Consideration]: https://img.shields.io/badge/Once-Under%20Consideration-cc7070?style=flat-square
[C API: Under Consideration]: https://img.shields.io/badge/C%20API-Under%20Consideration-cc7070?style=flat-square

[POSIX: Supported]: https://img.shields.io/badge/POSIX-Supported-success?style=flat-square
[Windows: Under Consideration]: https://img.shields.io/badge/Windows-Under%20Consideration-cc7070?style=flat-square
[Armv8-M Mainline (no CMSE): Supported]: https://img.shields.io/badge/Armv8--M%20Mainline%20(no%20CMSE)-Supported-success?style=flat-square
[Armv8-M Baseline (no CMSE): Supported]: https://img.shields.io/badge/Armv8--M%20Baseline%20(no%20CMSE)-Supported-success?style=flat-square
[Armv7-M: Supported]: https://img.shields.io/badge/Armv7--M-Supported-success?style=flat-square
[Armv6-M: Supported]: https://img.shields.io/badge/Armv6--M-Supported-success?style=flat-square
[Armv7-A (no FPU): Supported]: https://img.shields.io/badge/Armv7--A%20(no%20FPU)-Supported-success?style=flat-square
[RV32IMACFD: Supported]: https://img.shields.io/badge/RV32I%5BM%5DAC%5BFD%5D-Supported-success?style=flat-square
[RV64IMACFD: Supported]: https://img.shields.io/badge/RV64I%5BM%5DAC%5BFD%5D-Supported-success?style=flat-square

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

See [this document](examples/basic_gr_peach/README.md) for how to run the example application on [the GR-PEACH development board].

[the GR-PEACH development board]: https://www.renesas.com/us/en/products/gadget-renesas/boards/gr-peach.html#overview

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
 - [QEMU](https://www.qemu.org/) 4.2 or later to test the Arm-M/-A port.
 - libusb 1.x to test the Arm-M/-A port.
 - [OpenOCD](http://openocd.org) to test the Arm-A port on GR-PEACH.
 - `JLinkExe`<sup>†</sup> from [J-Link Software] to test the RISC-V port on RED-V.

[rustup]: https://rustup.rs/
[J-Link Software]: https://www.segger.com/downloads/jlink#J-LinkSoftwareAndDocumentationPack

[Nix] users can use the provided `shell.nix` file to install all required software except for those marked with <sup>†</sup>.

[Nix]: https://nixos.org/nix/

### How to Run Tests

 - Hosted platform and target-independent tests: `cargo test --all`
 - The Armv7-M port and NUCLEO-F401RE: `cargo run -p constance_test_runner -- -t nucleo_f401re`
 - The Armv7-M port and Arm MPS2+ AN385 (QEMU emulation): `cargo run -p constance_test_runner -- -t qemu_mps2_an385`
 - The Armv6-M port and Arm MPS2+ AN385 (QEMU emulation): `cargo run -p constance_test_runner -- -t qemu_mps2_an385_v6m`
 - The Armv7-A port and GR-PEACH: `cargo run -p constance_test_runner -- -t gr_peach`
 - The Armv7-A port and Arm RealView Platform Baseboard Explore for Cortex-A9 (QEMU emulation): `cargo run -p constance_test_runner -- -t qemu_realview_pbx_a9`
 - The RV32IMAC port and SiFive E (QEMU emulation): `cargo run -p constance_test_runner -- -t qemu_sifive_e_rv32`
 - The RV32GC port and SiFive U (QEMU emulation): `cargo run -p constance_test_runner -- -t qemu_sifive_u_rv32`
 - The RV64IMAC port and SiFive E (QEMU emulation): `cargo run -p constance_test_runner -- -t qemu_sifive_e_rv64`
 - The RV64GC port and SiFive U (QEMU emulation): `cargo run -p constance_test_runner -- -t qemu_sifive_u_rv64`
 - The RV32IMAC port and RED-V (SPI flash XIP): `cargo run -p constance_test_runner -- -t red_v`

### How to Run Benchmarks

The `-b` option instructs `constance_test_runner` to run benchmark tests. Note that some targets (notably QEMU Arm-M machines, which lack DWT) don't support benchmarking and the test code might crash, stall, or simply fail to compile on such targets.

 - Hosted platform: `cargo bench -p constance_port_std`
 - The Armv7-M port and NUCLEO-F401RE: `cargo run -p constance_test_runner -- -t nucleo_f401re -b`
 - The Armv7-A port and GR-PEACH: `cargo run -p constance_test_runner -- -t gr_peach -b`
 - The RV32IMAC port and RED-V (SPI flash XIP): `cargo run -p constance_test_runner -- -t red_v -b`
