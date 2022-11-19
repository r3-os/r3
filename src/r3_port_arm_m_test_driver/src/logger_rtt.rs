use core::{cell::RefCell, fmt::Write};
use cortex_m::interrupt;

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
            let Some(channel) = &mut *log_channel else { return; };
            writeln!(
                channel,
                "[{level:5} {target}] {args}",
                level = record.level(),
                target = record.target(),
                args = record.args()
            )
            .unwrap();
        });
    }

    fn flush(&self) {}
}

pub fn init(channel: rtt_target::UpChannel) {
    interrupt::free(move |cs| {
        *LOG_CHANNEL.borrow(cs).borrow_mut() = Some(channel);
    });
    log::set_logger(&Logger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
}
