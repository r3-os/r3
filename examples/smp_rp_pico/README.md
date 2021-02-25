This directory contains an example R3 application for [Raspberry Pi Pico] that utilizes two processor cores.

[Raspberry Pi Pico]: https://pico.raspberrypi.org/

Please see [`../basic_rp_pico/README.md`](../basic_rp_pico/README.md) for how to run this application.

You should see the following output through the virtual USB serial device:

```none
 0   | core0: 1.4s
   1 |                  core1: 1.5s
   1 |                  core1: 2s
 0   | core0: 2.1s
   1 |                  core1: 2.5s
 0   | core0: 2.8s
   1 |                  core1: 3s
   1 |                  core1: 3.5s
 0   | core0: 3.5s
   1 |                  core1: 4s
 0   | core0: 4.2s
   1 |                  core1: 4.5s
 0   | core0: 4.9s
   1 |                  core1: 5s
   1 |                  core1: 5.5s
```

You should also see the LED flashing in synchronous with core1's messages.

## Design

            core0                                        core1
          ,------------------------,                  ,------------------------,
          | ,--------------------, |                  | ,--------------------, |
          | | core0 kernel       | |                  | | core1 kernel       | |
          | |   ,------------,   | |     SIO FIFO     | |      ,-------,     | |
    USB <-------|  USB UART  |<--------------------------------| task1 |     | |
          | |   '------------'   | |                  | |      '-------'     | |
          | |      ↑      ↑      | |                  | '--------------------' |
          | | ,-------,,-------, | |                  |           ↑            |
          | | | task1 || task2 | | |     SIO FIFO     |  ,------------------,  |
          | | '-------''-------' | |          ,--------->| wait for payload |  |
          | |  ,--------------,  | |          |       |  '------------------'  |
          | |  | startup hook |---------------'       |           ↑            |
          | |  '--------------'  | |                  |      ,---------,       |
          | '--------------------' |                  |      | bootrom |       |
          |           ↑            |                  |      '---------'       |
          |    ,--------------,    |                  '------------------------'
          |    | cortex-m-rt  |    |
          |    | startup code |    |
          |    '--------------'    |
          |           ↑            |
          |    ,-------------,     |
          |    | boot loader |     |
          |    '-------------'     |
          |           ↑            |
          |      ,---------,       |
          |      | bootrom |       |
          |      '---------'       |
          '------------------------'

Our RTOS kernel does not support multi-core systems at the moment. This example utilizes multiple cores anyway by instantiating the kernel for each core. This is trivial to do because our kernel's implementation is inherently generic over "system" types, and we just have to create a system type for each kernel instantiation. In this example application, there are two such system types: `crate::{core0, core1}::System`.

There is some amount of unsafety that is present in this multi-core setup and must be handled carefully. For instance, [`cortex_m::interrupt::Mutex`] does not guarantee mutual exclusion across multiple processor cores. Using kernel objects created for one core in another will lead to problems, too. Module encapsulation might serve as a safeguard but is not perfect.

[`cortex_m::interrupt::Mutex`]: https://docs.rs/cortex-m/0.7.1/cortex_m/interrupt/struct.Mutex.html
