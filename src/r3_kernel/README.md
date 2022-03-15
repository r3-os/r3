# r3_kernel

The original kernel of [R3-OS][1].

- Traditional uniprocessor tickless real-time kernel with preemptive scheduling

- Implements a software-based scheduler supporting a customizable number of task priorities (up to 2¹⁵ levels on a 32-bit target, though the implementation is heavily optimized for a smaller number of priorities) and an unlimited number of tasks.

- Provides a scalable kernel timing mechanism with a logarithmic time complexity. This implementation is robust against a large interrupt processing delay.

- The kernel is split into a target-independent portion and a target-specific portion. The target-specific portion (called *a port*) is provided as a separate crate (e.g., [`r3_port_riscv`][2]). An application **combines them using the trait system**.

[1]: https://crates.io/crates/r3
[2]: https://crates.io/crates/r3_port_riscv
