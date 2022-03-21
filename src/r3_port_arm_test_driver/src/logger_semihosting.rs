struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        arm_semihosting::heprintln!(
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
    log::set_logger(&Logger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
}
