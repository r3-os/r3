# The Constance RTOS

Constance is a proof-of-concept of a static RTOS that utilizes Rust's compile-time function evaluation mechanism for static configuration (creation of kernel objects and memory allocation).

- All kernel objects are defined statically for faster boot times, compile-time checking, predictable execution, reduced RAM consumption, no runtime allocation failures, and extra security.
- The kernel and its configurator don't require an external build tool or a specialized procedural macro, maintaining transparency.
- The kernel doesn't include any code specific to a particular target. The target-specific portion (called *a port*) is provided as a separate crate, which an application chooses and combines with the kernel using the trait system.
- Leverages Rust's type safety for access control of kernel objects. Safe code can't access an object that it doesn't own.

## Implementation Status

|       Core       |     Library     |       Ports       |
| :--------------- | :-------------- | :---------------- |
| ☑︎ Tasks          | ☐ `Mutex`       | ☑︎ `std` (Hosted)  |
| ☑︎ Hunks          | ☐ `RwLock`      | ☐ Armv7-M         |
| ☑︎ Wait Objects   | ☐ `Once`        |                   |
| ☐ Time Events    | ☐ Logger        |                   |
| ☐ Semaphores     | ☐ C API         |                   |
| ☑︎ Event Groups   |                 |                   |
| ☐ Mutexes        | **Tools**       | **Boards**        |
| ☐ Timer          | ☑︎ Test Harness  | ☑︎ Hosted          |
| ☐ Alarm          | ☑︎ Test Suite    | ☐ F401RE          |
| ☐ Interrupts     | ☑︎ Configurator  |                   |
| ☐ CPU Exceptions |                 |                   |
| ☐ Panicking      |                 |                   |

## Example

```rust
use constance::kernel::Task;

// Use the simulator port
constance_port_std::use_port!(unsafe struct System);

struct Objects {
    task: Task<System>,
}

const COTTAGE: Objects = constance::build!(System, configure_app);

constance::configure! {
    fn configure_app(_: &mut CfgBuilder<System>) -> Objects {
        Objects {
            task: build! { Task<_>,
                start = task_body, priority = 2, active = true },
        }
    }
}

fn task_body(_: usize) {
    // The simulator initializes `env_logger` automatically
    log::warn!("yay");
}
```

## Getting Started

```shell
# Start the "basic" example application using the simulator port
$ cargo run -p constance_example_basic
```
