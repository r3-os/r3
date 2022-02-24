<h1 align="center" style="border: none">

![R3 Real-Time Operating System][]

</h1>

<style type="text/css">
body.theme-dark h1 img:nth-of-type(1) { filter: brightness(8) hue-rotate(-120deg) invert(90%) saturate(2.8) brightness(1); }
body.theme-ayu h1 img:nth-of-type(1) { filter: brightness(8) hue-rotate(-120deg) invert(90%) saturate(2.8) brightness(0.9); }
</style>

**R3-OS** (or simply **R3**) is a proof-of-concept of a static RTOS that utilizes Rust's compile-time function evaluation mechanism for static configuration (creation of kernel objects and memory allocation).

- **All kernel objects are defined statically** for faster boot times, compile-time checking, predictable execution, reduced RAM consumption, no runtime allocation failures, and extra security.
- A kernel and its configurator **don't require an external build tool or a specialized procedural macro**, maintaining transparency and inter-crate composability.
- The kernel API is **not tied to any specific kernel implementations**. Kernels are provided as separate crates, one of which an application chooses and instantiates using the trait system.
- Leverages Rust's type safety for access control of kernel objects. Safe code can't access an object that it doesn't own.

See [`r3_core`]'s crate-level documentation for a general description of kernel features and concepts used in R3.

# Package Ecosystem

The `r3` package is built on top on [`r3_core`] and provides useful abstractions. A kernel, which is chosen by application code, is implemented for a specific major version of `r3_core` and not others. Separating the fast-changing part into `r3` prevents ecosystem split and maximizes code reuse.

<table class="package-ecosystem-table" align="center">
    <tr>
        <th>Application code</th>
        <td colspan="2">Application</td>
        <td colspan="2">Library</td>
        <td colspan="2">Library</td>
        <td class="noborder">...</td>
    </tr>
    <tr>
        <th>Façade API</th>
        <td colspan="3"><code>r3 1.0¹</code></td>
        <td colspan="3"><code>r3 2.0</code></td>
        <td class="noborder">...</td>
    </tr>
    <tr>
        <th>Core API definition</th>
        <td colspan="7"><code>r3_core 1.0</code></td>
    </tr>
    <tr>
        <th>Kernel (API implementor)²</th>
        <td colspan="7"><code>r3_kernel</code></td>
    </tr>
    <tr>
        <th>Ports²</th>
        <td colspan="2"><code>r3_port_std</code></td>
        <td colspan="2"><code>r3_port_arm</code></td>
        <td colspan="2"><code>r3_port_arm_m</code></td>
        <td><code>r3_port_riscv</code></td>
    </tr>
</table>

<sub>¹ Version numbers shown here are illustrative.</sub>

<sub>² Application code chooses kernel and port implementations.</sub>

<style type="text/css">
.package-ecosystem-table { border-collapse: separate; border-spacing: 5px !important; }
.package-ecosystem-table td { border: 0.5px currentColor solid !important; text-align: center; vertical-align: middle }
.package-ecosystem-table td.noborder,
.package-ecosystem-table th { border: none !important; }
</style>

<div class="admonition-follows"></div>

> **Notes:** Many items of this crate are re-exported from [`r3_core`][], and consequently their example code refers to them through paths starting with `r3_core::`. You can replace them with `r3::` in your application code.

<!-- Display a "some Cargo features are disabled" warning in the documentation so that the user can know some items are missing for that reason. But we don't want this message to be displayed when someone is viewing `lib.md` directly, so the actual message is rendered by CSS. -->
<div class="admonition-follows"></div>
<blockquote class="disabled-feature-warning"><p><span></span><code></code></p></blockquote>

# Cargo Features

 - **`sync`** exports [`r3::sync`](crate::sync).

This package also exposes the Cargo features of [`r3_core`][]. Please refer to [its documentation][1].

[1]: r3_core#cargo-features
