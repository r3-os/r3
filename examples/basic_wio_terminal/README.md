This directory contains an example R3 application for [Wio Terminal][1].

Build the application by `cargo build --release`.

See <https://github.com/atsamd-rs/atsamd/blob/master/boards/wio_terminal/examples/README.md#wio-terminal-examples> for how to flash the application. The recommended way is to use [cargo-hf2][2], but you need a version that has [hf2-rs#44][1] merged:

```shell
cargo install cargo-hf2 --git https://github.com/yvt/hf2-rs.git --rev 3b0743d0d7fd4005973e6b44f45b391f05336bed
cargo hf2 --release --vid 0x2886 --pid 0x002d
```

[1]: https://github.com/jacobrosenthal/hf2-rs/pull/44
