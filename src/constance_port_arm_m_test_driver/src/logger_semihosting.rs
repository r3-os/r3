struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        cortex_m_semihosting::heprintln!(
            "[{:5} {}] {}",
            record.level(),
            record.target(),
            record.args()
        )
        .unwrap();
    }

    fn flush(&self) {}
}

pub fn init() {
    log::set_logger(&Logger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
}
