This directory contains an example R3 application for [GR-PEACH].

[GR-PEACH]: https://www.renesas.com/us/en/products/gadget-renesas/boards/gr-peach.html#overview

You need the following software to perform the steps described in this document:

 - [OpenOCD]
 - `arm-none-eabi-gdb`
 - [rustup] or the correct version of the Rust toolchain
 - The Armv7-A build of the Rust standard library (can be [installed] by `rustup target add armv7a-none-eabi`)

[OpenOCD]: http://openocd.org
[rustup]: https://rustup.rs
[installed]: https://rust-lang.github.io/rustup/cross-compilation.html

We are going to load this application onto the target using OpenOCD and GDB. In one terminal window, start an instance of OpenOCD using the supplied configuration file:

```shell
openocd -f opencd.cfg
```

In another terminal window, compile and load the application by doing the following:

```shell
cargo build --release
arm-none-eabi-gdb -iex 'target remote localhost:3333' ../../target/armv7a-none-eabi/release/basic_gr_peach
```

GDB automatically executes the commands in the `.gdbinit` file to download the image to the target's on-chip memory and direct PC to the entry point. Now, open a serial terminal application, configure the baud rate to 115200, and enter `c` in GDB. You should see the following output in the serial terminal:

```text
UART is ready
COTTAGE = Objects { task1: Task(1), task2: Task(2) }
time = 5.178ms
time = 1.006644s
time = 2.007444s
...
```
