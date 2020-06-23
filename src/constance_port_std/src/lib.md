Simulator for running [`::constance`] on a hosted environment

# Usage

```rust
#![feature(const_loop)]
#![feature(const_fn)]
#![feature(const_if_match)]
#![feature(const_mut_refs)]

// Require `unsafe` even in `unsafe fn` - highly recommended
#![feature(unsafe_block_in_unsafe_fn)]
#![deny(unsafe_op_in_unsafe_fn)]

use constance::kernel::Task;

// Use the simulator port
constance_port_std::use_port!(unsafe struct System);

const COTTAGE: () = constance::build!(System, configure_app);

constance::configure! {
    const fn configure_app(_: &mut CfgBuilder<System>) -> () {
        new! { Task<_>, start = task_body, priority = 1, active = true };
    }
}

fn task_body(_: usize) {
    // The simulator initializes `env_logger` automatically
    log::warn!("yay");
}
```
