use core::{cell::RefCell, fmt::Write};
use riscv::interrupt;

static LOG_CHANNEL: interrupt::Mutex<RefCell<Option<rtt_target::UpChannel>>> =
    interrupt::Mutex::new(RefCell::new(None));

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        interrupt::free(move |cs| {
            let mut log_channel = LOG_CHANNEL.borrow(cs).borrow_mut();
            if let Some(channel) = &mut *log_channel {
                writeln!(
                    channel,
                    "[{:5} {}] {}",
                    record.level(),
                    record.target(),
                    record.args()
                )
                .unwrap();
            }
        });
    }

    fn flush(&self) {}
}

pub fn init(channel: rtt_target::UpChannel) {
    interrupt::free(move |cs| {
        *LOG_CHANNEL.borrow(cs).borrow_mut() = Some(channel);
    });
    // Don't call `unwrap` to reduce the code size
    let _ = log::set_logger(&Logger);
    log::set_max_level(log::LevelFilter::Trace);
}
