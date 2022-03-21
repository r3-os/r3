struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        cortex_m_semihosting::heprintln!(
            "[{level:5} {target}] {args}",
            level = record.level(),
            target = record.target(),
            args = record.args()
        )
        .unwrap();
    }

    fn flush(&self) {}
}

pub fn init() {
    // Note: Some targets don't support CAS atomics. This is why we need to use
    //       `set_logger_racy` here.
    // Safety: There are no other threads calling `set_logger_racy` at the
    //         same time.
    unsafe { log::set_logger_racy(&Logger).unwrap() };
    log::set_max_level(log::LevelFilter::Trace);
}
