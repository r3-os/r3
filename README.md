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
#![feature(const_fn)]
#![feature(const_mut_refs)]
#![feature(const_fn_fn_ptr_basics)]
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

Start the `basic` example application using [the NUCLEO-F401RE board](https://www.st.com/en/evaluation-tools/nucleo-f401re.html) and [`cargo-embed`](https://crates.io/crates/cargo-embed) 0.9 for flashing:

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
 - libusb 1.x and libudev to run `constance_test_runner` (used to test various ports).
 - [OpenOCD](http://openocd.org) to test the Arm-A port on GR-PEACH.
 - `JLinkExe`<sup>†</sup> from [J-Link Software] to test the RISC-V port on RED-V.

[rustup]: https://rustup.rs/
[J-Link Software]: https://www.segger.com/downloads/jlink#J-LinkSoftwareAndDocumentationPack

[Nix] users can use the provided `shell.nix` file to install all required software except for those marked with <sup>†</sup>.

[Nix]: https://nixos.org/nix/

### How to Run Tests

| Architecture    |                  Board                   |                                Command                                   |
| --------------- | ---------------------------------------- | ------------------------------------------------------------------------ |
| Host            | Host                                     | `cargo test --all`                                                       |
| Armv7-M+FPU+DSP | [NUCLEO-F401RE]                          | `cargo run -p constance_test_runner -- -t nucleo_f401re`                 |
| Armv8-MML+FPU   | [Arm MPS2+]​ [AN505]​ (QEMU)               | `cargo run -p constance_test_runner -- -t qemu_mps2_an505`               |
| Armv8-MML       | Arm MPS2+ AN505 (QEMU)                   | `cargo run -p constance_test_runner -- -t qemu_mps2_an505 -a cortex_m33` |
| Armv8-MBL       | Arm MPS2+ AN505 (QEMU)                   | `cargo run -p constance_test_runner -- -t qemu_mps2_an505 -a cortex_m23` |
| Armv7-M         | Arm MPS2+ [AN385]​ (QEMU)                 | `cargo run -p constance_test_runner -- -t qemu_mps2_an385`               |
| Armv6-M         | Arm MPS2+ AN385 (QEMU)                   | `cargo run -p constance_test_runner -- -t qemu_mps2_an385 -a cortex_m0`  |
| Armv7-A         | [GR-PEACH]                               | `cargo run -p constance_test_runner -- -t gr_peach`                      |
| Armv7-A         | [Arm RealView PBX for Cortex-A9]​ (QEMU)  | `cargo run -p constance_test_runner -- -t qemu_realview_pbx_a9`          |
| RV32IMAC        | [SiFive E]​ (QEMU)                        | `cargo run -p constance_test_runner -- -t qemu_sifive_e_rv32`            |
| RV32GC          | [SiFive U]​ (QEMU)                        | `cargo run -p constance_test_runner -- -t qemu_sifive_u_rv32`            |
| RV64IMAC        | SiFive E (QEMU)                          | `cargo run -p constance_test_runner -- -t qemu_sifive_e_rv64`            |
| RV64GC          | SiFive U (QEMU)                          | `cargo run -p constance_test_runner -- -t qemu_sifive_u_rv64`            |
| RV32IMAC        | [RED-V]​ (SPI flash XIP)                  | `cargo run -p constance_test_runner -- -t red_v`                         |
| RV64GC          | [Maix] boards (UART ISP)                 | `cargo run -p constance_test_runner -- -t maix`                          |

[NUCLEO-F401RE]: https://www.st.com/en/evaluation-tools/nucleo-f401re.html
[Arm MPS2+]: https://developer.arm.com/tools-and-software/development-boards/fpga-prototyping-boards/mps2
[AN505]: http://infocenter.arm.com/help/topic/com.arm.doc.dai0505b/index.html
[AN385]: https://developer.arm.com/documentation/dai0385/d/
[GR-PEACH]: https://www.renesas.com/us/en/products/gadget-renesas/boards/gr-peach.html
[Arm RealView PBX for Cortex-A9]: https://developer.arm.com/docs/dui0440/latest/preface
[SiFive E]: https://github.com/sifive/freedom-e-sdk
[SiFive U]: https://github.com/sifive/freedom-u-sdk
[RED-V]: https://www.sparkfun.com/products/15594?_ga=2.171541280.1047902909.1599963676-1377824336.1599963676
[Maix]: https://maixduino.sipeed.com/en/

### How to Run Benchmarks

The `-b` option instructs `constance_test_runner` to run benchmark tests. Note that some targets (notably QEMU Arm-M machines, which lack DWT) don't support benchmarking and the test code might crash, stall, or simply fail to compile on such targets.

| Architecture |         Board          |                           Command                           |
| ------------ | ---------------------- | ----------------------------------------------------------- |
| Host         | Host                   | `cargo bench -p constance_port_std`                         |
| Armv7-M      | NUCLEO-F401RE          | `cargo run -p constance_test_runner -- -t nucleo_f401re -b` |
| Armv7-A      | GR-PEACH               | `cargo run -p constance_test_runner -- -t gr_peach -b`      |
| RV32IMAC     | RED-V (SPI flash XIP)  | `cargo run -p constance_test_runner -- -t red_v -b`         |
| RV64GC       | Maix boards (UART ISP) | `cargo run -p constance_test_runner -- -t maix -b`          |
