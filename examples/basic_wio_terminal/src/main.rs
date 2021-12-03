#![feature(asm)]
#![feature(const_fn_trait_bound)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_mut_refs)]
#![feature(let_else)]
#![feature(const_trait_impl)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unsupported_naked_functions)]
#![no_std]
#![no_main]
#![cfg(target_os = "none")]

use embedded_graphics as eg;
use r3_port_arm_m as port;
use wio_terminal as wio;

use core::{
    cell::RefCell,
    fmt::Write,
    panic::PanicInfo,
    sync::atomic::{AtomicUsize, Ordering},
};
use cortex_m::{interrupt::Mutex as PrimaskMutex, singleton};
use eg::{image::Image, mono_font, pixelcolor::Rgb565, prelude::*, primitives, text};
use r3::{
    kernel::{InterruptLine, InterruptNum, Mutex, StartupHook, Task, Timer},
    prelude::*,
};
use spin::Mutex as SpinMutex;
use usb_device::{
    bus::UsbBusAllocator,
    device::{UsbDevice, UsbDeviceBuilder, UsbVidPid},
};
use usbd_serial::{SerialPort, USB_CLASS_CDC};
use wio::{
    hal::{clock::GenericClockController, delay::Delay, gpio, usb::UsbBus},
    pac::{CorePeripherals, Peripherals},
    prelude::*,
    Pins, Sets,
};

// Port configuration
// -------------------------------------------------------------------------

type System = r3_kernel::System<SystemTraits>;
port::use_port!(unsafe struct SystemTraits);
port::use_systick_tickful!(unsafe impl PortTimer for SystemTraits);

impl port::ThreadingOptions for SystemTraits {}

impl port::SysTickOptions for SystemTraits {
    const FREQUENCY: u64 = 120_000_000; // ??
    const TICK_PERIOD: u32 = Self::FREQUENCY as u32 / 500; // 2ms
}

/// This part is `port::use_rt!` minus `__INTERRUPTS`. `wio_terminal`'s default
/// feature set includes `atsamd-hal/samd51p-rt`, which produces a conflicting
/// definition of `__INTERRUPTS`. However, when the default feature set is
/// disabled (`default-features = false`), `wio_terminal` fails to compile. So
/// the only option left ot us is to suppress our `__INTERRUPTS`.
const _: () = {
    use port::{rt::imp::ExceptionTrampoline, EntryPoint, INTERRUPT_SYSTICK};
    use r3_kernel::KernelCfg2;

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
            ExceptionTrampoline::new(<SystemTraits as EntryPoint>::HANDLE_PEND_SV);

        unsafe { <SystemTraits as EntryPoint>::start() };
    }

    #[cortex_m_rt::exception]
    fn SysTick() {
        if let Some(x) = <SystemTraits as KernelCfg2>::INTERRUPT_HANDLERS.get(INTERRUPT_SYSTICK) {
            // Safety: It's a first-level interrupt handler here. CPU Lock inactive
            unsafe { x() };
        }
    }
};

// Application
// ----------------------------------------------------------------------------

struct Objects {
    console_pipe: queue::Queue<System, u8>,
    lcd_mutex: Mutex<System>,
    button_reporter_task: Task<System>,
    usb_in_task: Task<System>,
    usb_poll_timer: Timer<System>,
    usb_interrupt_lines: [InterruptLine<System>; 3],
}

const COTTAGE: Objects = r3_kernel::build!(SystemTraits, configure_app => Objects);

/// The top-level configuration function.
const fn configure_app(b: &mut r3_kernel::Cfg<SystemTraits>) -> Objects {
    b.num_task_priority_levels(4);

    // Register a hook to initialize hardware
    StartupHook::define()
        .start(|_| {
            init_hardware();
        })
        .finish(b);

    // Register a timer driver initializer
    SystemTraits::configure_systick(b);

    // Miscellaneous tasks
    let _noisy_task = Task::define()
        .start(noisy_task_body)
        .priority(0)
        .active(true)
        .finish(b);
    let button_reporter_task = Task::define()
        .start(button_reporter_task_body)
        .priority(2)
        .active(true)
        .finish(b);
    let _blink_task = Task::define()
        .start(blink_task_body)
        .priority(1)
        .active(true)
        .finish(b);

    // USB input handler
    let usb_in_task = Task::define()
        .start(usb_in_task_body)
        .priority(2)
        .active(true)
        .finish(b);
    let usb_poll_timer = Timer::define()
        .start(usb_poll_timer_handler)
        .delay(r3::time::Duration::from_millis(0))
        // Should be < 10ms for USB compliance
        .period(r3::time::Duration::from_millis(5))
        .finish(b);
    let usb_interrupt_lines = [
        InterruptLine::define()
            .line(interrupt::USB_OTHER as InterruptNum + port::INTERRUPT_EXTERNAL0)
            .priority(1)
            .enabled(true)
            .finish(b),
        InterruptLine::define()
            .line(interrupt::USB_TRCPT0 as InterruptNum + port::INTERRUPT_EXTERNAL0)
            .priority(1)
            .enabled(true)
            .finish(b),
        InterruptLine::define()
            .line(interrupt::USB_TRCPT1 as InterruptNum + port::INTERRUPT_EXTERNAL0)
            .priority(1)
            .enabled(true)
            .finish(b),
    ];

    // Graphics-related tasks and objects
    let _animation_task = Task::define()
        .start(animation_task_body)
        .priority(2)
        .active(true)
        .finish(b);
    let _console_task = Task::define()
        .start(console_task_body)
        .priority(3)
        .active(true)
        .finish(b);
    let console_pipe = queue::Queue::new(b);
    let lcd_mutex = Mutex::define().finish(b);

    Objects {
        console_pipe,
        lcd_mutex,
        button_reporter_task,
        usb_in_task,
        usb_poll_timer,
        usb_interrupt_lines,
    }
}

static LCD: SpinMutex<Option<wio::LCD>> = SpinMutex::new(None);
static BLINK_ST: SpinMutex<Option<BlinkSt>> = SpinMutex::new(None);

struct BlinkSt {
    user_led: gpio::Pin<gpio::v2::pin::PA15, gpio::v2::Output<gpio::v2::PushPull>>,
}

fn init_hardware() {
    let mut peripherals = Peripherals::take().unwrap();
    let mut core_peripherals = unsafe { CorePeripherals::steal() };

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

    // Register button event handlers
    let button_ctrlr = sets.buttons.init(
        peripherals.EIC,
        &mut clocks,
        &mut peripherals.MCLK,
        &mut sets.port,
    );
    button_ctrlr.enable(&mut core_peripherals.NVIC);
    unsafe { BUTTON_CTRLR = Some(button_ctrlr) };

    // Configure the USB serial device
    let sets_usb = sets.usb;
    let peripherals_usb = peripherals.USB;
    let peripherals_mclk = &mut peripherals.MCLK;
    let usb_bus_allocator = singleton!(
        : UsbBusAllocator<UsbBus> =
        sets_usb.usb_allocator(
            peripherals_usb,
            &mut clocks,
            peripherals_mclk,
        )
    )
    .unwrap();
    let serial = SerialPort::new(usb_bus_allocator);
    let usb_device = UsbDeviceBuilder::new(usb_bus_allocator, UsbVidPid(0x16c0, 0x27dd))
        .product("R3 Example")
        .device_class(USB_CLASS_CDC)
        .max_packet_size_0(64)
        .build();
    *USB_STDIO_GLOBAL.lock() = Some(UsbStdioGlobal { serial, usb_device });
}

// Message producer
// ----------------------------------------------------------------------------

/// The task responsible for outputting messages to the console.
fn noisy_task_body(_: usize) {
    let _ = writeln!(Console, "////////////////////////////////");
    let _ = writeln!(
        Console,
        "Hello! Send text to me over the USB serial port \
        (e.g., `/dev/ttyACM0`), and I'll display it!"
    );
    let _ = writeln!(Console, "////////////////////////////////");
    loop {
        // Print a message
        let _ = write!(Console, "-- {:?} --", System::time().unwrap());

        System::sleep(r3::time::Duration::from_secs(60)).unwrap();
        let _ = writeln!(Console);
    }
}

// Console and graphics
// ----------------------------------------------------------------------------

/// Acquire a lock on `wio::LCD`, yielding the CPU to lower-priority tasks as
/// necessary.
///
/// Do not do `LCD.lock()` directly - it monopolizes CPU time, possibly causing
/// a dead lock.
fn borrow_lcd() -> impl core::ops::DerefMut<Target = wio::LCD> {
    struct Guard(Option<spin::MutexGuard<'static, Option<wio::LCD>>>);

    impl Drop for Guard {
        fn drop(&mut self) {
            self.0 = None;
            COTTAGE.lcd_mutex.unlock().unwrap();
        }
    }

    impl core::ops::Deref for Guard {
        type Target = wio::LCD;

        #[inline]
        fn deref(&self) -> &Self::Target {
            self.0.as_ref().unwrap().as_ref().unwrap()
        }
    }

    impl core::ops::DerefMut for Guard {
        #[inline]
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.0.as_mut().unwrap().as_mut().unwrap()
        }
    }

    COTTAGE.lcd_mutex.lock().unwrap();
    Guard(Some(LCD.lock()))
}

struct Console;

impl Write for Console {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        COTTAGE.console_pipe.write(s.as_bytes());
        Ok(())
    }
}

/// The task responsible for blinking the user LED.
fn blink_task_body(_: usize) {
    let mut st = BLINK_ST.lock();
    let st = st.as_mut().unwrap();
    loop {
        st.user_led.toggle();
        System::sleep(r3::time::Duration::from_millis(210)).unwrap();
    }
}

/// The task responsible for rendering the console.
fn console_task_body(_: usize) {
    let mut lcd = borrow_lcd();

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
    let num_cols: usize = 200 / char_width;
    let num_rows: usize = 240 / char_height;

    primitives::Rectangle::with_corners(Point::new(0, 0), Point::new(320, 320))
        .into_styled(bg_style)
        .draw(&mut *lcd)
        .unwrap();

    drop(lcd);

    loop {
        let num_read = COTTAGE.console_pipe.read(&mut buf);

        let mut lcd = borrow_lcd();
        let lcd = &mut *lcd;

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
                    Point::new(
                        (num_cols * char_width) as i32,
                        (char_height * cursor[1] + char_height) as i32,
                    ),
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

/// The task responsible for rendering an animated image.
fn animation_task_body(_: usize) {
    let images = r3_example_common::ANIMATION_FRAMES_565;
    for image in images.iter().cycle() {
        let mut lcd = borrow_lcd();
        Image::new(&image(), Point::new(320 - 10 - 102, 240 - 10 - 86))
            .draw(&mut *lcd)
            .unwrap();
        drop(lcd);

        System::sleep(r3::time::Duration::from_millis(20)).unwrap();
    }
}

// Button listener
// ----------------------------------------------------------------------------

static BUTTON_STATE: AtomicUsize = AtomicUsize::new(0);

/// The task responsible for reporting button events.
fn button_reporter_task_body(_: usize) {
    let mut st = 0;
    loop {
        System::park().unwrap();

        let new_st = BUTTON_STATE.load(Ordering::Relaxed);

        // Report changes in the button state
        use wio::Button;
        for (i, b) in [
            Button::TopLeft,
            Button::TopMiddle,
            Button::Down,
            Button::Up,
            Button::Left,
            Button::Right,
            Button::Click,
        ]
        .iter()
        .enumerate()
        {
            // `assert_eq!(*b as usize, i)`, but if we did this, `b` would be
            // lost because it's not `Copy` (why???)
            let mask = 1 << i;
            if (st ^ new_st) & mask != 0 {
                let _ = write!(
                    Console,
                    "{:?}: {}",
                    b,
                    ["UP", "DOWN"][(new_st & mask != 0) as usize]
                );
            }
        }

        st = new_st;
    }
}

static mut BUTTON_CTRLR: Option<wio::ButtonController> = None;

// These are all needed by `wio::button_interrupt!`
use cortex_m::interrupt::{free as disable_interrupts, CriticalSection};
use wio::{pac::interrupt, ButtonEvent};

wio::button_interrupt! {
    BUTTON_CTRLR,
    unsafe fn on_button_event(_cs: &CriticalSection, event: ButtonEvent) {
        // We can't call kernel methods while `PRIMASK` is set
        // (Lesson: Poorly designed abstraction can (motivate people to)
        // undermine Rust's safety mechanism.)
        // Safety: la la la
        unsafe { cortex_m::interrupt::enable() };

        let mut st = BUTTON_STATE.load(Ordering::Relaxed);
        if event.down {
            st |= 1<<event.button as u32;
        } else {
            st &= !(1<<event.button as u32);
        }
        BUTTON_STATE.store(st, Ordering::Relaxed);

        // Report the event
        COTTAGE.button_reporter_task.unpark().unwrap();

        cortex_m::interrupt::disable();
    }
}

// USB serial
// ----------------------------------------------------------------------------

struct UsbStdioGlobal {
    usb_device: UsbDevice<'static, UsbBus>,
    serial: SerialPort<'static, UsbBus>,
}

/// Stores [`UsbStdioGlobal`]. Only accessed by `poll_usb` (the USB interrupt
/// handler).
static USB_STDIO_GLOBAL: SpinMutex<Option<UsbStdioGlobal>> = SpinMutex::new(None);

/// The USB input queue size
const USB_BUF_CAP: usize = 64;

/// The queue through which received data is passed from `poll_usb` to
/// `usb_in_task_body`
static USB_BUF_IN: PrimaskMutex<RefCell<([u8; USB_BUF_CAP], usize)>> =
    PrimaskMutex::new(RefCell::new(([0; USB_BUF_CAP], 0)));

/// USB interrupt handler
fn poll_usb() {
    let Some(mut g) = USB_STDIO_GLOBAL.try_lock() else { return };
    let g = g.as_mut().unwrap();

    // It's important that we poll the USB device frequently enough
    g.usb_device.poll(&mut [&mut g.serial]);

    let mut should_unpark = false;
    let mut should_start_polling = false;

    disable_interrupts(|cs| {
        let mut usb_buf_in = USB_BUF_IN.borrow(cs).borrow_mut();
        let (buf, buf_len) = &mut *usb_buf_in;
        let remaining = &mut buf[*buf_len..];
        if remaining.is_empty() {
            // We can't process the data fast enough; apply back-pressure.
            // Also, disable the USB interrupt lines because we would otherwise
            // get an interrupt storm. (I'm surprised we have to do this. Is
            // this really the proper way to apply back-pressure?)
            should_start_polling = true;
            return;
        }

        if let Ok(len) = g.serial.read(remaining) {
            assert!(len <= remaining.len());
            *buf_len += len;
            should_unpark = len > 0;
        }
    });

    // In this configuration `disable_interrupts` is equivalent to CPU Lock, so
    // kernel functions cannot be called inside it
    if should_unpark {
        COTTAGE.usb_in_task.unpark().unwrap();
    }

    if should_start_polling {
        set_usb_polling(true);
    }
}

#[interrupt]
fn USB_OTHER() {
    poll_usb();
}

#[interrupt]
fn USB_TRCPT0() {
    poll_usb();
}

#[interrupt]
fn USB_TRCPT1() {
    poll_usb();
}

fn usb_poll_timer_handler(_: usize) {
    poll_usb();
}

/// Change whether `poll_usb` is called in response to USB interrupts or in a
/// constant interval.
fn set_usb_polling(b: bool) {
    if b {
        COTTAGE.usb_poll_timer.start().unwrap();
        for line in COTTAGE.usb_interrupt_lines.iter() {
            line.disable().unwrap();
        }
    } else {
        COTTAGE.usb_poll_timer.stop().unwrap();
        for line in COTTAGE.usb_interrupt_lines.iter() {
            line.enable().unwrap();
        }
    }
}

/// The task to print the data received by the USB serial endpoint
fn usb_in_task_body(_: usize) {
    let mut data = arrayvec::ArrayVec::<u8, USB_BUF_CAP>::new();
    loop {
        // Get next data to output
        disable_interrupts(|cs| {
            let mut usb_buf_in = USB_BUF_IN.borrow(cs).borrow_mut();
            let (buf, buf_len) = &mut *usb_buf_in;
            data.clear();
            data.try_extend_from_slice(&buf[..*buf_len]).unwrap();
            *buf_len = 0;
        });

        if data.is_empty() {
            // Got nothing; sleep until new data arrives
            System::park().unwrap();
            continue;
        }

        if data.is_full() {
            set_usb_polling(false);
        }

        // Send it to the console
        let data = core::str::from_utf8(&data).unwrap_or("");
        let _ = Console.write_str(data);
    }
}

// Utilities
// ----------------------------------------------------------------------------

mod queue {
    use r3::{
        kernel::{traits, Cfg, Kernel, Task},
        sync::mutex::Mutex,
        utils::Init,
    };

    pub trait SupportedSystem: traits::KernelMutex + traits::KernelStatic {}
    impl<T: traits::KernelMutex + traits::KernelStatic> SupportedSystem for T {}

    pub struct Queue<System: SupportedSystem, T> {
        st: Mutex<System, QueueSt<System, T>>,
        reader_lock: Mutex<System, ()>,
        writer_lock: Mutex<System, ()>,
    }

    const CAP: usize = 256;

    struct QueueSt<System: SupportedSystem, T> {
        buf: [T; CAP],
        read_i: usize,
        len: usize,
        waiting_reader: Option<Task<System>>,
        waiting_writer: Option<Task<System>>,
    }

    impl<System: SupportedSystem, T: Init> Init for QueueSt<System, T> {
        const INIT: Self = Self {
            buf: [T::INIT; CAP],
            read_i: 0,
            len: 0,
            waiting_reader: None,
            waiting_writer: None,
        };
    }

    impl<System: SupportedSystem, T: Init + Copy + 'static> Queue<System, T> {
        pub const fn new<C>(cfg: &mut Cfg<C>) -> Self
        where
            C: ~const traits::CfgBase<System = System> + ~const traits::CfgMutex,
        {
            Self {
                st: Mutex::define().finish(cfg),
                reader_lock: Mutex::define().finish(cfg),
                writer_lock: Mutex::define().finish(cfg),
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
