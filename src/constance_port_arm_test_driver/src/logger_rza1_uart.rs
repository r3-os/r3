use constance_support_rza1::serial::ScifExt;

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        constance_support_rza1::sprintln!(
            "[{:5} {}] {}",
            record.level(),
            record.target(),
            record.args()
        );
    }

    fn flush(&self) {}
}

pub fn init() {
    let rza1::Peripherals {
        CPG, GPIO, SCIF2, ..
    } = unsafe { rza1::Peripherals::steal() };

    SCIF2.enable_clock(&CPG);
    SCIF2.configure_pins(&GPIO);
    SCIF2.configure_uart(115200);

    constance_support_rza1::stdout::set_stdout(SCIF2.into_nb_writer());

    log::set_logger(&Logger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
}
