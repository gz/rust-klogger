#![no_std]
#![crate_name = "klogger"]
#![crate_type = "lib"]

use core::fmt;

#[macro_use]
pub mod macros;

extern crate log;
extern crate termcodes;

#[cfg(target_arch = "x86_64")]
extern crate x86;

#[cfg(target_arch = "x86_64")]
#[path = "arch/x86.rs"]
mod arch;

use log::{Level, Metadata, Record, SetLoggerError};
use termcodes::color;

static mut LOGGER: KLogger = KLogger { start_tsc: 0 };

pub struct Writer;

impl Writer {
    /// Obtain a logger for the specified module.
    pub fn get_module(module: &str) -> Writer {
        use core::fmt::Write;
        let mut ret = Writer;
        write!(&mut ret, "[{}] ", module).expect("Writer");
        ret
    }

    pub fn get() -> Writer {
        Writer
    }
}

impl ::core::ops::Drop for Writer {
    /// Release the logger.
    fn drop(&mut self) {
        use core::fmt::Write;
        write!(self, "\n").expect("Newline");
    }
}

impl fmt::Write for Writer {
    /// Write stuff to serial out.
    fn write_str(&mut self, s: &str) -> fmt::Result {
        unsafe {
            arch::puts(s);
        }
        Ok(())
    }
}

pub struct WriterNoDrop;

impl WriterNoDrop {
    pub fn get() -> WriterNoDrop {
        WriterNoDrop
    }
}

impl fmt::Write for WriterNoDrop {
    /// Write stuff to serial out.
    fn write_str(&mut self, s: &str) -> fmt::Result {
        unsafe {
            arch::puts(s);
        }
        Ok(())
    }
}

struct KLogger {
    start_tsc: u64,
}

impl log::Log for KLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let cur = unsafe { x86::time::rdtsc() };

            let color = match record.level() {
                Level::Error => color::AnsiValue(202),
                Level::Warn => color::AnsiValue(167),
                Level::Info => color::AnsiValue(136),
                Level::Debug => color::AnsiValue(64),
                Level::Trace => color::AnsiValue(32),
            };

            sprintln!(
                "{}{:>10}{} [{}{:5}{}] - {}: {}{}{}",
                color::Fg(color::LightYellow),
                cur - self.start_tsc,
                color::Fg(color::Reset),
                color::Fg(color),
                record.level(),
                color::Fg(color::Reset),
                record.target(),
                color::Fg(color::LightWhite),
                record.args(),
                color::Fg(color::Reset),
            );
        }
    }

    fn flush(&self) {}
}

pub fn init(level: Level) -> Result<(), SetLoggerError> {
    unsafe {
        (&mut LOGGER).start_tsc = x86::time::rdtsc();
        log::set_logger(&LOGGER).map(|()| log::set_max_level(level.to_level_filter()))
    }
}
