use log::{Metadata, Record};

pub static CONSOLE_LOGGER: ConsoleLogger = ConsoleLogger;

pub struct ConsoleLogger;

impl log::Log for ConsoleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("[{}]\t{}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}
