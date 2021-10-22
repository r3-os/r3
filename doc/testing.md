# Running Tests and Benchmarks

This document explains how to use the test suite and what is needed to do so.

## Prerequisites

 - [rustup], which will automatically install the version of Nightly Rust compiler specified by `rust-toolchain`
 - [QEMU](https://www.qemu.org/) 4.2 or later to test the Arm-M/-A port.
 - libusb 1.x and libudev to run `r3_test_runner` (used to test various ports).
 - [OpenOCD](http://openocd.org) to test the Arm-A port on GR-PEACH.
 - `JLinkExe`<sup>†</sup> from [J-Link Software] to test the RISC-V port on RED-V.

[rustup]: https://rustup.rs/
[J-Link Software]: https://www.segger.com/downloads/jlink#J-LinkSoftwareAndDocumentationPack

[Nix] users can use the provided `shell.nix` file to install all required software except for those marked with <sup>†</sup>.

[Nix]: https://nixos.org/nix/

## How to Run Tests

`cargo test --all` runs all tests including the kernel test suite (with all optional features enabled) on the host environment.

The following table shows how to run the kernel test suite for each target.

|   Architecture  |                  Board                   |                                Command                                |
| --------------- | ---------------------------------------- | --------------------------------------------------------------------- |
| Host            | Host                                     | `cargo test -p r3_port_std --features r3_test_suite/full`             |
| Armv7-M+FPU+DSP | [NUCLEO-F401RE]                          | `cargo run -p r3_test_runner -- -t nucleo_f401re`                     |
| Armv8-MML+FPU   | [Arm MPS2+]​ [AN505]​ (QEMU)             | `cargo run -p r3_test_runner -- -t qemu_mps2_an505`                   |
| Armv8-MML       | Arm MPS2+ AN505 (QEMU)                   | `cargo run -p r3_test_runner -- -t qemu_mps2_an505 -a cortex_m33`     |
| Armv8-MBL       | Arm MPS2+ AN505 (QEMU)                   | `cargo run -p r3_test_runner -- -t qemu_mps2_an505 -a cortex_m23`     |
| Armv7-M         | Arm MPS2+ [AN385]​ (QEMU)                | `cargo run -p r3_test_runner -- -t qemu_mps2_an385`                   |
| Armv6-M         | Arm MPS2+ AN385 (QEMU)                   | `cargo run -p r3_test_runner -- -t qemu_mps2_an385 -a cortex_m0`      |
| Armv6-M         | [Raspberry Pi Pico]​ (USB)               | `cargo run -p r3_test_runner -- -t rp_pico`                           |
| Armv7-A         | [GR-PEACH]                               | `cargo run -p r3_test_runner -- -t gr_peach`                          |
| Armv7-A         | [Arm RealView PBX for Cortex-A9]​ (QEMU) | `cargo run -p r3_test_runner -- -t qemu_realview_pbx_a9`              |
| RV32IMAC        | [SiFive E]​ (QEMU)                       | `cargo run -p r3_test_runner -- -t qemu_sifive_e_rv32`                |
| RV32GC          | [SiFive U]​ (QEMU)                       | `cargo run -p r3_test_runner -- -t qemu_sifive_u_rv32`                |
| RV64IMAC        | SiFive U (QEMU)                          | `cargo run -p r3_test_runner -- -t qemu_sifive_u_rv64 -a rv64i+m+a+c` |
| RV64GC          | SiFive U (QEMU)                          | `cargo run -p r3_test_runner -- -t qemu_sifive_u_rv64`                |
| RV32IMAC        | [RED-V]​ (SPI flash XIP)                 | `cargo run -p r3_test_runner -- -t red_v`                             |
| RV64GC          | [Maix] boards (UART ISP)                 | `cargo run -p r3_test_runner -- -t maix`                              |

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
[Raspberry Pi Pico]: https://pico.raspberrypi.org/

## How to Run Benchmarks

The `-b` option instructs `r3_test_runner` to run benchmark tests. Note that some targets (notably QEMU Arm-M machines, which lack DWT) don't support benchmarking and the test code might crash, stall, or simply fail to compile on such targets.

| Architecture |          Board          |                           Command                           |
| ------------ | ----------------------- | ----------------------------------------------------------- |
| Host         | Host                    | `cargo bench -p r3_port_std`                         |
| Armv7-M      | NUCLEO-F401RE           | `cargo run -p r3_test_runner -- -t nucleo_f401re -b` |
| Armv7-A      | GR-PEACH                | `cargo run -p r3_test_runner -- -t gr_peach -b`      |
| Armv6-M      | Raspberry Pi Pico (USB) | `cargo run -p r3_test_runner -- -t rp_pico -b`       |
| RV32IMAC     | RED-V (SPI flash XIP)   | `cargo run -p r3_test_runner -- -t red_v -b`         |
| RV64GC       | Maix boards (UART ISP)  | `cargo run -p r3_test_runner -- -t maix -b`          |


## Configuring udev

If you are using a Linux (or similar) system, you might have to [define][] additional udev rules to allow non-root access to the target board.

> **Note:** The rules apply when a device is plugged into the computer. You might need to reconnect the device for the changes to take effect.

[define]: https://wiki.archlinux.org/title/udev

### Raspberry Pi Pico

For this board, the test suite uses the USB PICOBOOT interface (provided by the bootrom) and the USB-CDC interface (provided by R3's test driver).

For the PICOBOOT interface, the following rule changes the device file's group to `users`:

```
SUBSYSTEM=="usb", ATTRS{idVendor}=="2e8a", ATTRS{idProduct}=="0003", GROUP="users"
```

For the USB-CDC interface, there is usually [a pre-defined udev rule][] that designates `dialout` as the device file's group. You can add yourself to the `dialout` group by doing `sudo usermod -a -G dialout yourUserName`.

[a pre-defined udev rule]: https://unix.stackexchange.com/questions/395464/permissions-incorrect-on-symlink-to-dev-ttyacm0-created-by-udev-rule

### NUCLEO-F401RE (ST-LINK)

The following rule changes the device file's group to `users`:

```
SUBSYSTEM=="usb", ATTRS{idVendor}=="0483", ATTRS{idProduct}=="374b", GROUP="users"
```
