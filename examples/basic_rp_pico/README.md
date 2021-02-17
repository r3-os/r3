This directory contains an example R3 application for [Raspberry Pi Pico].

[Raspberry Pi Pico]: https://pico.raspberrypi.org/

You need the following software to run this example.

 - `arm-none-eabi-gcc` to build [the second-stage bootloader](https://crates.io/crates/rp2040-boot2)
     - Try `nix-shell -p gcc-arm-embedded` if you use Nix(OS)
 - [`elf2uf2`](https://github.com/raspberrypi/pico-sdk/tree/master/tools/elf2uf2) or [`elf2uf2-rs`](https://github.com/jonil/elf2uf2-rs)

First, place your Pico into BOOTSEL mode by holding the BOOTSEL button down while connecting it to your computer. You should see a volume named `RPI-RP2`  being mounted if your Pico has successfully entered BOOTSEL mode.

After that, run the following commands (assuming `elf2uf2-rs` is installed):

```shell
cd examples/basic_rp_pico
cargo build --release
elf2uf2-rs -d ../../target/thumbv6m-none-eabi/release/r3_example_basic_rp_pico
```

You should see the on-board LED blinking after doing this. This program presents your Pico as a USB serial device. If you open it by a serial monitor app, you should see some textual output.
