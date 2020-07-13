The Arm-M port for [Constance](::constance).

TODO

# Interrupts

(Logical) interrupt lines (`InterruptNum`) are mapped to Arm-M exceptions associated with identical exception numbers. The first external interrupt is associated with the interrupt number `16` ([`INTERRUPT_EXTERNAL0`]).

The interrupt numbers `0..16` are mapped to non-external interrupts, and most operations that are usually supported with external interrupts such as enabling, pending, and even registering a handler are not supported for these interrupt numbers because either they don't make sense or they can interfere with the port's correct operation. The only exception here is SysTick ([`INTERRUPT_SYSTICK`]). SysTick supports pending, clearing, and registering a handler, but not enabling or disabling.

[`INTERRUPT_EXTERNAL0`]: crate::INTERRUPT_EXTERNAL0
[`INTERRUPT_SYSTICK`]: crate::INTERRUPT_SYSTICK

# Safety

Being a low-level piece of software, this port directly interfaces with hardware. This is not a problem as long as the port is the only piece of code doing that, but it might interfere with other low-level libraries and break their assumptions, potentially leading to an undefined behavior. This section lists potential harmful interactions that an application developer should keep in mind.

As a general thumb rule, you should not directly access hardware registers (e.g., `BASEPRI` and `CONTROL`) and peripherals (e.g., NVIC) that the port uses or exposes a standardized interface to access. You should access them only though the operating system.

## Interaction with `::cortex_m`

This port agrees with `::cortex_m` in that updating `PRIMASK` is `unsafe` because using it incorrectly can break a certain type of critical section.

## Stack Overflow

This port doesn't support detecting stack overflow.
