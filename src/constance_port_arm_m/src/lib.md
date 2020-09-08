The Arm-M port for [Constance](::constance).

# Interrupts

(Logical) interrupt lines (`InterruptNum`) are mapped to Arm-M exceptions associated with identical exception numbers. The first external interrupt is associated with the interrupt number `16` ([`INTERRUPT_EXTERNAL0`]).

The interrupt numbers `0..16` are mapped to non-external interrupts, and most operations that are usually supported with external interrupts such as enabling, pending, and even registering a handler are not supported for these interrupt numbers because either they don't make sense or they can interfere with the port's correct operation. The only exception here is SysTick ([`INTERRUPT_SYSTICK`]). SysTick supports pending, clearing, and registering a handler, but not enabling or disabling.

[`INTERRUPT_EXTERNAL0`]: crate::INTERRUPT_EXTERNAL0
[`INTERRUPT_SYSTICK`]: crate::INTERRUPT_SYSTICK

# Kernel Timing

The availability of timer sources varies greatly between MCUs and there's no one-size-fits-all solution. For this reason, [`use_port!`] does not implement [`PortTimer`] on your system type. The Arm-M architecture defines SysTick, an optional timer integrated with a processor core and most Arm-M-based MCUs are equipped with those. This crate provides an implementation of `PortTimer` that utilizes SysTick.

[`PortTimer`]: constance::kernel::PortTimer

## Tickful SysTick

This implementation is selected by [`use_systick_tickful!`]. It configures SysTick to fire at a constant interval. The SysTick handler advances the tick count by a constant amount every time it's called.

**Pros:** The time measurement is as accurate as the source clock.

**Cons:** Preempts tasks frequently. Inefficient in terms of energy consumption. Timeout precision is limited by the tick frequency. Can't tolerate a large interrupt delay (missing one interrupt is enough to disrupt the time measurement).

## Tickless SysTick

TODO

# Idle Task

When there is no task to schedule, the port transfers the control to **the idle task** (this is an internal construct and invisible to the kernel or an application). The idle task executes the `wfi` instruction to reduce power consumption.

The use of the `wfi` instruction can interfere with debugger connection. For example, RTT (Real-Time Transfer) stops working when the processor of STM32F401 is idle. Setting [`ThreadingOptions::USE_WFI`] to `false` solves this issue.

[`ThreadingOptions::USE_WFI`]: crate::ThreadingOptions::USE_WFI

# Register Preloading

When a task is activated, a new context state is created inside the task's stack. By default, only essential registers are preloaded with known values. The **`preload-registers`** Cargo feature enables preloading for all integer registers, which might help in debugging at the cost of performance and code size.

# Safety

Being a low-level piece of software, this port directly interfaces with hardware. This is not a problem as long as the port is the only piece of code doing that, but it might interfere with other low-level libraries and break their assumptions, potentially leading to an undefined behavior. This section lists potential harmful interactions that an application developer should keep in mind.

As a general thumb rule, you should not directly access hardware registers (e.g., `BASEPRI` and `CONTROL`) and peripherals (e.g., NVIC) that the port uses or exposes a standardized interface to access. You should access them only though the operating system.

## Interaction with `::cortex_m`

This port agrees with `::cortex_m` in that updating `PRIMASK` is `unsafe` because using it incorrectly can break a certain type of critical section.

## Stack Overflow

This port doesn't support detecting stack overflow.
