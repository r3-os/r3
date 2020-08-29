struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        crate::uart::stderr_write_fmt(format_args!(
            "[{:5} {}] {}\n",
            record.level(),
            record.target(),
            record.args()
        ));
    }

    fn flush(&self) {}
}

pub fn init() {
    log::set_logger(&Logger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
}
