use log::Log;
use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};

type LogRecord = (log::Level, String, String);

pub struct CapturedLog {
    buf: Vec<LogRecord>,
}

impl CapturedLog {
    fn log(&mut self, record: &log::Record) {
        let mesg = if let Some(s) = record.args().as_str() {
            s.to_string()
        } else {
            record.args().to_string()
        };

        let r = (record.level(), record.target().to_owned(), mesg);
        self.buf.push(r);
    }

    fn take(&mut self) -> Vec<LogRecord> {
        self.buf.drain(..).collect()
    }
}

#[derive(Clone)]
pub struct TestLogger {
    captures: Arc<Mutex<CapturedLog>>,
}

impl Default for TestLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl TestLogger {
    pub fn new() -> TestLogger {
        TestLogger {
            captures: Arc::new(Mutex::new(CapturedLog { buf: Vec::new() })),
        }
    }

    pub fn take(&self) -> Vec<LogRecord> {
        let mut cap = self.captures.lock().unwrap();
        cap.take()
    }
}

impl Log for TestLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let mut cap = self.captures.lock().unwrap();
        cap.log(record);
    }

    fn flush(&self) {
        // do nothing
    }
}

static LOGGER: Lazy<TestLogger> = Lazy::new(|| {
    let logger = TestLogger::new();
    log::set_boxed_logger(Box::new(logger.clone())).unwrap();
    log::set_max_level(log::LevelFilter::Debug);
    logger
});

pub fn test_logger() -> TestLogger {
    LOGGER.clone()
}
