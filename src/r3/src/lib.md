<h1 align="center" style="border: none">

![R3 Real-Time Operating System][]

</h1>

<style type="text/css">
body.theme-dark h1 img:nth-of-type(1) { filter: brightness(8) hue-rotate(-120deg) invert(90%) saturate(2.8) brightness(1); }
body.theme-ayu h1 img:nth-of-type(1) { filter: brightness(8) hue-rotate(-120deg) invert(90%) saturate(2.8) brightness(0.9); }
</style>

R3 is a proof-of-concept of a static RTOS that utilizes Rust's compile-time function evaluation mechanism for static configuration (creation of kernel objects and memory allocation).

- **All kernel objects are defined statically** for faster boot times, compile-time checking, predictable execution, reduced RAM consumption, no runtime allocation failures, and extra security.
- A kernel and its configurator **don't require an external build tool or a specialized procedural macro**, maintaining transparency and inter-crate composability.
- The kernel API is **not tied to a specific kernel implementation**. Kernels are provided as separate crates, one of which an application chooses and instantiates using the trait system.
- Leverages Rust's type safety for access control of kernel objects. Safe code can't access an object that it doesn't own.

TODO

<!-- Display a "some Cargo features are disabled" warning in the documentation so that the user can know some items are missing for that reason. But we don't want this message to be displayed when someone is viewing `lib.md` directly, so the actual message is rendered by CSS. -->
<div class="admonition-follows"></div>
<blockquote class="disabled-feature-warning"><p><span></span><code></code></p></blockquote>

# Cargo Features

This package re-exports the Cargo features of [`r3_core`][]. Please refer to [its documentation][1].

[1]: r3_core#cargo-features
