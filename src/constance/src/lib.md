The Constance RTOS

[![Constance and Fluttershy](https://derpicdn.net/img/2018/5/25/1740985/medium.png)](http://derpibooru.org/1740985)

# Design

## Trait-based Composition

The Constance RTOS utilizes Rust's trait system to allow system designers to construct a system in a modular way.

The following pseudocode outlines the traits and types involved in hooking up the kernel, port, and application to each other.

```rust,ignore
crate constance {
    /// Implemented by a port.
    unsafe trait Port {
        type TaskState;
        fn dispatch();
        /* ... */
    }

    /// Associates `System` with kernel-private data. Implemented by `build!`.
    /// The kernel-private data includes port-specific types.
    unsafe trait KernelCfg: Port {
        const TASK_CFG: &'static [TaskCfg<Self::TaskState>];
        /* ... */
    }

    /// The API used by the application and the port. This is automatically
    /// implemented when a type has sufficient trait `impl`s.
    trait Kernel: Port + KernelCfg {}

    impl<T: Port + KernelCfg> Kernel for T { /* ... */ }

    /// Instantiate the `static`s necessary for the kernel's operation. This is
    /// absolutely impossible to do with blanket `impl`s.
    macro_rules! build {
        ($sys:ty, $configure:expr) => {
            unsafe impl $crate::KernelCfg for $sys {
                const TASK_CFG: &'static [TaskCfg<Self::TaskState>] = /* ... */;
                /* ... */
            }
        };
    }
}

crate constance_xxx_port {
    // The following approach doesn't work because of a circular dependency in
    // blanket `impl`s:
    //
    // impl<T: constance::Kernel> constance::Port for T {}

    // Instead, `Port` should be implemented specifically for a type. This is
    // facilitated by a macro, which also has an advantage of giving the port an
    // opportunity to insert port-specific code (such as `static`s and inline
    // assembler) referencing `$sys` to the application.
    macro_rules! use_port {
        (unsafe struct $sys:ident) => {
            struct $sys;

            // Assume `$sys: Kernel`
            unsafe impl constance::Port for $sys {
                /* ... */
            }
        };
    }
}

crate your_app {
    constance_xxx_port::use_port!(unsafe struct System);

    struct Objects {
        task1: constance::Task<System>,
    }

    static COTTAGE: Objects = constance::build!(System, configure_app);

    // The configure function. The exact syntax is yet to be determined. See
    // the section Static Configuration for more.
    constance::configure! {
        fn configure_app(_: CfgBuilder<System>) -> Objects {
            Objects {
                task1: new_task!(),
            }
        }
    }
}
```

## Static Configuration

Kernel objects are created in *a configure function*, defined by the [`configure!`] macro.

The syntax of configure functions is not determined at the moment and is expected to change as Rust's const evaluation capability matures.
