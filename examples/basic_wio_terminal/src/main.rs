#![feature(asm)]
#![feature(const_fn_trait_bound)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_mut_refs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unsupported_naked_functions)]
#![no_std]
#![no_main]
#![cfg(target_os = "none")]

use embedded_graphics as eg;
use r3_port_arm_m as port;
use wio_terminal as wio;

use core::{fmt::Write, panic::PanicInfo};
use eg::{mono_font, pixelcolor::Rgb565, prelude::*, primitives, text};
use r3::{
    kernel::{cfg::CfgBuilder, StartupHook, Task},
    prelude::*,
};
use spin::Mutex as SpinMutex;
use wio::{
    hal::{clock::GenericClockController, delay::Delay, gpio},
    pac::{CorePeripherals, Peripherals},
    prelude::*,
    Pins, Sets,
};

// Port configuration
// -------------------------------------------------------------------------

port::use_port!(unsafe struct System);
port::use_systick_tickful!(unsafe impl PortTimer for System);

impl port::ThreadingOptions for System {}

impl port::SysTickOptions for System {
    const FREQUENCY: u64 = 120_000_000; // ??
}

/// This part is `port::use_rt!` minus `__INTERRUPTS`. `wio_terminal`'s default
/// feature set includes `atsamd-hal/samd51p-rt`, which produces a conflicting
/// definition of `__INTERRUPTS`. However, when the default feature set is
/// disabled (`default-features = false`), `wio_terminal` fails to compile. So
/// the only option left ot us is to suppress our `__INTERRUPTS`.
const _: () = {
    use port::{rt::imp::ExceptionTrampoline, EntryPoint, INTERRUPT_SYSTICK};
    use r3::kernel::KernelCfg2;

    #[cortex_m_rt::entry]
    fn main() -> ! {
        unsafe {
            asm!(
                "
                    .global PendSV
                    PendSV = {} + 1
                ",
                sym PEND_SV_TRAMPOLINE
            );
        }

        #[link_section = ".text"]
        static PEND_SV_TRAMPOLINE: ExceptionTrampoline =
            ExceptionTrampoline::new(<System as EntryPoint>::HANDLE_PEND_SV);

        unsafe { <System as EntryPoint>::start() };
    }

    #[cortex_m_rt::exception]
    fn SysTick() {
        if let Some(x) = <System as KernelCfg2>::INTERRUPT_HANDLERS.get(INTERRUPT_SYSTICK) {
            // Safety: It's a first-level interrupt handler here. CPU Lock inactive
            unsafe { x() };
        }
    }
};

// Application
// ----------------------------------------------------------------------------

struct Objects {
    console_pipe: queue::Queue<System, u8>,
}

const COTTAGE: Objects = r3::build!(System, configure_app => Objects);

const fn configure_app(b: &mut CfgBuilder<System>) -> Objects {
    b.num_task_priority_levels(4);

    // Initialize hardware
    StartupHook::build()
        .start(|_| {
            init_hardware();
        })
        .finish(b);

    System::configure_systick(b);

    Task::build()
        .start(noisy_task_body)
        .priority(1)
        .active(true)
        .finish(b);
    Task::build()
        .start(blink_task_body)
        .priority(2)
        .active(true)
        .finish(b);
    Task::build()
        .start(console_task_body)
        .priority(3)
        .active(true)
        .finish(b);

    let console_pipe = queue::Queue::new(b);

    Objects { console_pipe }
}

static LCD: SpinMutex<Option<wio::LCD>> = SpinMutex::new(None);
static BLINK_ST: SpinMutex<Option<BlinkSt>> = SpinMutex::new(None);

struct BlinkSt {
    user_led: gpio::Pin<gpio::v2::pin::PA15, gpio::v2::Output<gpio::v2::PushPull>>,
}

fn init_hardware() {
    let mut peripherals = Peripherals::take().unwrap();
    let core_peripherals = unsafe { CorePeripherals::steal() };

    // Configure the clock tree
    let mut clocks = GenericClockController::with_external_32kosc(
        peripherals.GCLK,
        &mut peripherals.MCLK,
        &mut peripherals.OSC32KCTRL,
        &mut peripherals.OSCCTRL,
        &mut peripherals.NVMCTRL,
    );

    // Configure SysTick's clock input
    let mut delay = Delay::new(core_peripherals.SYST, &mut clocks);

    // Configure the user LED pin
    let mut sets: Sets = Pins::new(peripherals.PORT).split();
    let mut user_led = sets.user_led.into_open_drain_output(&mut sets.port);
    user_led.set_low().unwrap();

    *BLINK_ST.lock() = Some(BlinkSt { user_led });

    // Configure the LCD
    let (display, _backlight) = sets
        .display
        .init(
            &mut clocks,
            peripherals.SERCOM7,
            &mut peripherals.MCLK,
            &mut sets.port,
            58.mhz(),
            &mut delay,
        )
        .unwrap();

    *LCD.lock() = Some(display);
}

fn noisy_task_body(_: usize) {
    let mut msg = arrayvec::ArrayString::<80>::new();
    loop {
        // Print a message
        msg.clear();
        let _ = write!(msg, "time = {:?}", System::time().unwrap());
        COTTAGE.console_pipe.write(msg.as_bytes());

        for _ in 0..10 {
            COTTAGE.console_pipe.write(b".");
            System::sleep(r3::time::Duration::from_millis(55)).unwrap();
        }
        COTTAGE.console_pipe.write(b"\n");
    }
}

fn blink_task_body(_: usize) {
    let mut st = BLINK_ST.lock();
    let st = st.as_mut().unwrap();
    loop {
        st.user_led.toggle();
        System::sleep(r3::time::Duration::from_millis(210)).unwrap();
    }
}

fn console_task_body(_: usize) {
    let mut lcd = LCD.lock();
    let lcd = lcd.as_mut().unwrap();

    let bg_style = primitives::PrimitiveStyleBuilder::new()
        .fill_color(Rgb565::BLACK)
        .build();

    let char_style = mono_font::MonoTextStyleBuilder::new()
        .font(&mono_font::ascii::FONT_6X10)
        .text_color(Rgb565::WHITE)
        .background_color(Rgb565::BLACK)
        .build();
    let text_style = text::TextStyleBuilder::new()
        .baseline(text::Baseline::Top)
        .build();
    let mut buf = [0u8; 32];
    let mut cursor = [0usize; 2];
    let char_width: usize = 6;
    let char_height: usize = 10;
    let num_cols: usize = 320 / char_width;
    let num_rows: usize = 240 / char_height;

    primitives::Rectangle::with_corners(Point::new(0, 0), Point::new(320, 320))
        .into_styled(bg_style)
        .draw(lcd)
        .unwrap();

    loop {
        let num_read = COTTAGE.console_pipe.read(&mut buf);
        let mut buf = &buf[..num_read];
        while !buf.is_empty() {
            let i = buf.iter().position(|&b| b == b'\n').unwrap_or(buf.len());
            let mut span_len = i.min(num_cols - cursor[0]);
            let span = core::str::from_utf8(&buf[..span_len]).unwrap_or("");

            text::Text::with_text_style(
                span,
                Point::new(
                    2 + (char_width * cursor[0]) as i32,
                    (char_height * cursor[1]) as i32,
                ),
                char_style,
                text_style,
            )
            .draw(lcd)
            .unwrap();

            cursor[0] += span_len;
            if cursor[0] >= num_cols || i == 0 {
                cursor[0] = 0;
                cursor[1] += 1;
                if cursor[1] == num_rows {
                    cursor[1] = 0;
                }

                // Erase the new line
                primitives::Rectangle::with_corners(
                    Point::new(0, (char_height * cursor[1]) as i32),
                    Point::new(320, (char_height * cursor[1] + char_height) as i32),
                )
                .into_styled(bg_style)
                .draw(lcd)
                .unwrap();
            }
            if i == 0 {
                // Skip b'\n'
                span_len += 1;
            }
            buf = &buf[span_len..];
        }
    }
}

// Utilities
// ----------------------------------------------------------------------------

mod queue {
    use r3::{
        kernel::{cfg::CfgBuilder, Kernel, Task},
        sync::mutex::Mutex,
        utils::Init,
    };

    pub struct Queue<System, T> {
        st: Mutex<System, QueueSt<System, T>>,
        reader_lock: Mutex<System, ()>,
        writer_lock: Mutex<System, ()>,
    }

    const CAP: usize = 256;

    struct QueueSt<System, T> {
        buf: [T; CAP],
        read_i: usize,
        len: usize,
        waiting_reader: Option<Task<System>>,
        waiting_writer: Option<Task<System>>,
    }

    impl<System, T: Init> Init for QueueSt<System, T> {
        const INIT: Self = Self {
            buf: [T::INIT; CAP],
            read_i: 0,
            len: 0,
            waiting_reader: None,
            waiting_writer: None,
        };
    }

    impl<System: Kernel, T: Init + Copy + 'static> Queue<System, T> {
        pub const fn new(cfg: &mut CfgBuilder<System>) -> Self {
            Self {
                st: Mutex::build().finish(cfg),
                reader_lock: Mutex::build().finish(cfg),
                writer_lock: Mutex::build().finish(cfg),
            }
        }

        pub fn read(&self, out_buf: &mut [T]) -> usize {
            let _guard = self.reader_lock.lock().unwrap();
            loop {
                let mut st_guard = self.st.lock().unwrap();
                let st = &mut *st_guard;
                if st.len == 0 {
                    // Block the current task while the buffer is empty
                    st.waiting_reader = Some(Task::current().unwrap().unwrap());
                    drop(st_guard);
                    System::park().unwrap();
                } else {
                    let copied = st.len.min(out_buf.len());
                    let (part1, part0) = st.buf.split_at(st.read_i);
                    if part0.len() >= copied {
                        out_buf[..copied].copy_from_slice(&part0[..copied]);
                    } else {
                        out_buf[..part0.len()].copy_from_slice(part0);
                        out_buf[part0.len()..copied]
                            .copy_from_slice(&part1[..copied - part0.len()]);
                    }
                    st.read_i = st.read_i.wrapping_add(copied) % CAP;
                    st.len -= copied;

                    // Wake up any waiting writer
                    if let Some(t) = st.waiting_writer.take() {
                        t.unpark().unwrap();
                    }

                    st.waiting_reader = None;
                    return copied;
                }
            }
        }

        pub fn write(&self, mut in_buf: &[T]) {
            let _guard = self.writer_lock.lock().unwrap();
            while !in_buf.is_empty() {
                let mut st_guard = self.st.lock().unwrap();
                let st = &mut *st_guard;
                let copied = (CAP - st.len).min(in_buf.len());
                if copied == 0 {
                    // Block the current task while the buffer is full
                    st.waiting_writer = Some(Task::current().unwrap().unwrap());
                    drop(st_guard);
                    System::park().unwrap();
                } else {
                    let (part1, part0) = st.buf.split_at_mut((st.read_i + st.len) % CAP);
                    if part0.len() >= copied {
                        part0[..copied].copy_from_slice(&in_buf[..copied]);
                    } else {
                        part0.copy_from_slice(&in_buf[..part0.len()]);
                        part1[..copied - part0.len()].copy_from_slice(&in_buf[part0.len()..copied]);
                    }
                    st.len += copied;

                    // Wake up any waiting reader
                    if let Some(t) = st.waiting_reader.take() {
                        t.unpark().unwrap();
                    }

                    st.waiting_writer = None;
                    in_buf = &in_buf[copied..];
                }
            }
        }
    }
}

// Fatal error handlers
// ----------------------------------------------------------------------------

/// The panic handler
#[inline(never)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let _ = System::acquire_cpu_lock();

    unsafe { LCD.force_unlock() };
    let mut lcd = LCD.lock();

    if let Some(lcd) = lcd.as_mut() {
        let mut msg = arrayvec::ArrayString::<256>::new();
        if let Err(_) = write!(msg, "panic: {}", info) {
            msg.clear();
            msg.push_str("panic: (could not format the message)");
        }

        let char_style = mono_font::MonoTextStyleBuilder::new()
            .font(&mono_font::ascii::FONT_6X10)
            .text_color(Rgb565::YELLOW)
            .background_color(Rgb565::BLACK)
            .build();
        let text_style = text::TextStyleBuilder::new()
            .baseline(text::Baseline::Top)
            .build();

        // If the panic message only contains ASCII characters, chunking by
        // bytes should be okay
        // TODO: handle line breaks correcly
        for (chunk, y) in msg
            .as_bytes()
            .chunks(320 / 6)
            .map(|bytes| core::str::from_utf8(bytes).unwrap_or("???"))
            .zip((0..).step_by(10))
        {
            let _ = text::Text::with_text_style(chunk, Point::new(2, y), char_style, text_style)
                .draw(lcd);
        }
    }

    loop {}
}

#[cortex_m_rt::exception]
fn DefaultHandler(x: i16) -> ! {
    panic!("unhandled exception {}", x);
}

#[cortex_m_rt::exception]
fn HardFault(fr: &cortex_m_rt::ExceptionFrame) -> ! {
    panic!("hard fault: {:?}", fr);
}
