This directory contains an example R3 application for [Wio Terminal][1].

Build the application by `cargo build --release`.

See <https://github.com/atsamd-rs/atsamd/blob/master/boards/wio_terminal/examples/README.md#wio-terminal-examples> for how to flash the application. The recommended way is to use [cargo-hf2][2] 0.3.3 or later (pre-[hf2-rs#44][1] versions will silently write a corrupted image):

```shell
cargo install cargo-hf2
cargo hf2 --release --vid 0x2886 --pid 0x002d
```

cargo-hf2 might panic upon flashing completion, but it can be ignored safely ([hf2-rs#38][2]).

[1]: https://github.com/jacobrosenthal/hf2-rs/pull/44
[2]: https://github.com/jacobrosenthal/hf2-rs/issues/38
