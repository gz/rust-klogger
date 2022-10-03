#![cfg_attr(not(target_family = "unix"), no_std)]
#![crate_name = "klogger"]
#![crate_type = "lib"]

#[cfg(not(target_os = "none"))]
extern crate core;
extern crate heapless;
extern crate pl011_qemu;

use core::fmt;
use core::ops;

#[macro_use]
pub mod macros;

extern crate log;
extern crate termcodes;

#[cfg(any(
    feature = "use_ioports",
    all(target_arch = "x86_64", target_os = "none")
))]
#[path = "arch/x86.rs"]
mod arch;

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
#[path = "arch/aarch64.rs"]
mod arch;

#[cfg(all(target_arch = "aarch64", feature = "use_ioports"))]
compile_error!("ioports are not supported on aarch64");

#[cfg(all(not(feature = "use_ioports"), target_family = "unix"))]
#[path = "arch/unix.rs"]
mod arch;

use heapless::{String, Vec};
use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};
use termcodes::color; // type level integer used to specify capacity

/// Global lock to protect serial line from concurrent printing.
pub static SERIAL_LINE_MUTEX: spin::Mutex<bool> = spin::Mutex::new(false);

#[derive(Debug)]
pub struct Directive {
    name: Option<String<64>>,
    level: LevelFilter,
}

//unsafe impl ArrayLength<Directive> for Directive;

#[derive(Debug)]
struct KLogger {
    /// Do we even have a TSC?
    ///
    /// If not bad.
    has_tsc: bool,
    /// Is the underlying TSC invariant?
    ///
    /// If not, bad.
    has_invariant_tsc: bool,
    /// Point in time when this Klogger got initialized
    tsc_start: u64,
    /// Frequency in Hz
    ///
    /// Sometimes we can't figure this out (yet)
    tsc_frequency: Option<u64>,
    /// Filter(s) used by Klogger.
    ///
    /// Use module name or log level or both for filtering.
    filter: Vec<Directive, 8>,
}

enum ElapsedTime {
    Undetermined,
    Nanoseconds(u64),
    Cycles(u64),
}

impl fmt::Display for ElapsedTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ElapsedTime::Nanoseconds(ns) => write!(f, "{:>10}", ns),
            // the cyc is added here so we are made aware it's not nano-seconds
            ElapsedTime::Cycles(cycles) => write!(f, "{:>10} cyc", cycles),
            ElapsedTime::Undetermined => write!(f, ""),
        }
    }
}

impl KLogger {
    /// Time in nano seconds since KLogger init.
    fn elapsed(&self) -> ElapsedTime {
        if self.has_tsc {
            let cur = arch::get_timestamp();

            if self.has_invariant_tsc && self.tsc_frequency.is_some() {
                let elapsed_cycles = cur - self.tsc_start;
                let _tsc_frequency_hz = self.tsc_frequency.unwrap_or(1); // This won't fail, checked by if above

                // Basic is: let ns = elapsed_cycles / (tsc_frequency / NS_PER_SEC);
                // But we avoid removing all precision with division:
                // TODO: fix overflow with * NS_PER_SEC
                //let ns = (elapsed_cycles * NS_PER_SEC) / tsc_frequency_hz;
                let ns = elapsed_cycles;

                ElapsedTime::Nanoseconds(ns)
            } else {
                // We can't convert cycles to a time
                ElapsedTime::Cycles(cur)
            }
        } else {
            // We don't know
            ElapsedTime::Undetermined
        }
    }

    /// Returns the maximum `LevelFilter` that this filter instance is
    /// configured to output.
    pub fn filter(&self) -> LevelFilter {
        return self
            .filter
            .iter()
            .map(|d| d.level)
            .max()
            .unwrap_or(LevelFilter::Off);
    }
}

impl log::Log for KLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let level = metadata.level();
        let target = metadata.target();

        enabled(&self.filter, level, target)
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let color = match record.level() {
                Level::Error => color::AnsiValue(202),
                Level::Warn => color::AnsiValue(167),
                Level::Info => color::AnsiValue(136),
                Level::Debug => color::AnsiValue(64),
                Level::Trace => color::AnsiValue(32),
            };

            sprintln!(
                "{}{}{} [{}{:5}{}] - {}: {}{}{}",
                color::Fg(color::LightYellow),
                self.elapsed(),
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

static mut LOGGER: KLogger = KLogger {
    has_tsc: false,
    has_invariant_tsc: false,
    tsc_start: 0,
    tsc_frequency: None,
    filter: Vec::new(),
};

/// A writer for the serial line. It holds a lock so
/// multiple cores/threads can print at the same time.
pub struct Writer<'a> {
    /// Lock on the serial line, it is implicitly released on a drop.
    #[allow(dead_code)]
    line_lock: spin::MutexGuard<'a, bool>,
}

impl<'a> Writer<'a> {
    /// Obtain a logger for the specified module.
    pub fn get_module(module: &str) -> Writer<'a> {
        use core::fmt::Write;
        let line_lock = SERIAL_LINE_MUTEX.lock();
        let mut ret = Writer { line_lock };
        write!(&mut ret, "[{}] ", module).expect("Writer");
        ret
    }

    /// Obtain a logger.
    pub fn get() -> Writer<'a> {
        let line_lock = SERIAL_LINE_MUTEX.lock();
        Writer { line_lock }
    }
}

impl<'a> ops::Drop for Writer<'a> {
    /// Release the logger (and the line_lock), end output with a newline.
    ///
    /// Serial standard mandates the use of '\r\n' for a newline and
    /// resetting the curser to the beginning.
    fn drop(&mut self) {
        use core::fmt::Write;
        #[allow(clippy::write_with_newline)]
        write!(self, "\r\n").expect("Newline");
    }
}

impl<'a> fmt::Write for Writer<'a> {
    /// Write stuff to serial out.
    fn write_str(&mut self, s: &str) -> fmt::Result {
        unsafe {
            arch::puts(s);
        }
        Ok(())
    }
}

/// A writer that doesn't respect the locking procedure and tries to write at all costs.
///
/// It's used by sprint at the moment. It can also be useful as part of panics handlers
/// where we really want to print in all circumstances.
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

pub fn init(args: &str, output_indicator: u16) -> Result<(), SetLoggerError> {
    arch::set_output(output_indicator);

    unsafe {
        LOGGER.has_tsc = arch::has_tsc();
        LOGGER.has_invariant_tsc = arch::has_invariant_tsc();

        if LOGGER.has_tsc {
            LOGGER.tsc_start = arch::get_timestamp();
        }

        let tsc_frequency_hz: Option<u64> = arch::get_tsc_frequency_hz();

        // Check if we run in a VM and the hypervisor can give us the TSC frequency
        let vmm_tsc_frequency_hz: Option<u64> = arch::get_vmm_tsc_frequency_hz();

        if tsc_frequency_hz.is_some() {
            LOGGER.tsc_frequency = tsc_frequency_hz;
        } else if vmm_tsc_frequency_hz.is_some() {
            LOGGER.tsc_frequency = vmm_tsc_frequency_hz;
        }

        // Another way that segfaults in KVM:
        // The scalable bus frequency is encoded in the bit field MSR_PLATFORM_INFO[15:8]
        // and the nominal TSC frequency can be determined by multiplying this number
        // by a bus speed of 100 MHz.
        //(&mut LOGGER).tsc_frequency =
        //    ((x86::msr::rdmsr(x86::msr::MSR_PLATFORM_INFO) >> 8) & 0xff) * 1000000;

        parse_args(&mut LOGGER.filter, args);
        log::set_logger(&LOGGER).map(|()| log::set_max_level(LOGGER.filter()))
    }
}

pub fn putchar(c: char) {
    unsafe {
        arch::putc(c);
    }
}

/// Most of the filtering code is inspired or copied from
/// https://github.com/sebasmagri/env_logger/blob/master/src/filter/mod.rs
///
/// Parse a logging specification string (e.g: "crate1,crate2::mod3,crate3::x=error")
/// and return a vector with log directives.
fn parse_args(filter: &mut Vec<Directive, 8>, spec: &str) {
    let mut parts = spec.split('/');
    let mods = parts.next();
    if parts.next().is_some() {
        sprintln!(
            "warning: invalid logging spec '{}', \
             ignoring it (too many '/'s)",
            spec
        );
        return;
    }
    mods.map(|m| {
        for s in m.split(',') {
            if s.len() == 0 {
                continue;
            }
            let mut parts = s.split('=');
            let (log_level, name) =
                match (parts.next(), parts.next().map(|s| s.trim()), parts.next()) {
                    (Some(part0), None, None) => {
                        // if the single argument is a log-level string or number,
                        // treat that as a global fallback
                        match part0.parse() {
                            Ok(num) => (num, None),
                            Err(_) => (LevelFilter::max(), Some(part0)),
                        }
                    }
                    (Some(part0), Some(""), None) => (LevelFilter::max(), Some(part0)),
                    (Some(part0), Some(part1), None) => match part1.parse() {
                        Ok(num) => (num, Some(part0)),
                        _ => {
                            sprintln!(
                                "warning: invalid logging spec '{}', \
                                 ignoring it",
                                part1
                            );
                            continue;
                        }
                    },
                    _ => {
                        sprintln!(
                            "warning: invalid logging spec '{}', \
                             ignoring it",
                            s
                        );
                        continue;
                    }
                };

            match filter.push(Directive {
                name: match name {
                    None => None,
                    Some(name) => Some(String::from(name)),
                },
                level: log_level,
            }) {
                Ok(_) => {}
                Err(e) => {
                    sprintln!("Unable to add new filter {:?}", e);
                }
            }
        }
    });
}

// Check whether a level and target are enabled by the set of directives.
fn enabled(directives: &[Directive], level: Level, target: &str) -> bool {
    // Search for the longest match, the vector is assumed to be pre-sorted.
    for directive in directives.iter().rev() {
        match directive.name {
            Some(ref name) if !target.starts_with(&**name) => {}
            Some(..) | None => return level <= directive.level,
        }
    }
    false
}

#[cfg(test)]
mod test {
    use heapless::String;
    use heapless::Vec as VEC;
    use log::{Level, LevelFilter};

    use super::{enabled, parse_args, Directive};

    #[test]
    fn filter_info() {
        let mut filter: Vec<Directive> = Vec::new();
        filter.push(Directive {
            name: None,
            level: LevelFilter::Info,
        });
        assert!(enabled(&filter, Level::Info, "crate1"));
        assert!(!enabled(&filter, Level::Debug, "crate1"));
    }

    #[test]
    fn filter_beginning_longest_match() {
        let mut filter: Vec<Directive> = Vec::new();
        filter.push(Directive {
            name: Some(String::from("crate2")),
            level: LevelFilter::Info,
        });
        filter.push(Directive {
            name: Some(String::from("crate2::mod")),
            level: LevelFilter::Debug,
        });
        filter.push(Directive {
            name: Some(String::from("crate1::mod1")),
            level: LevelFilter::Warn,
        });
        assert!(enabled(&filter, Level::Debug, "crate2::mod1"));
        assert!(!enabled(&filter, Level::Debug, "crate2"));
    }

    #[test]
    fn parse_default() {
        let mut filter: VEC<Directive, 8> = VEC::new();
        parse_args(&mut filter, "info,crate1::mod1=warn");
        assert!(enabled(&filter, Level::Warn, "crate1::mod1"));
        assert!(enabled(&filter, Level::Info, "crate2::mod2"));
    }

    #[test]
    fn match_full_path() {
        let logger = vec![
            Directive {
                name: Some(String::from("crate2")),
                level: LevelFilter::Info,
            },
            Directive {
                name: Some(String::from("crate1::mod1")),
                level: LevelFilter::Warn,
            },
        ];
        assert!(enabled(&logger, Level::Warn, "crate1::mod1"));
        assert!(!enabled(&logger, Level::Info, "crate1::mod1"));
        assert!(enabled(&logger, Level::Info, "crate2"));
        assert!(!enabled(&logger, Level::Debug, "crate2"));
    }

    #[test]
    fn no_match() {
        let logger = vec![
            Directive {
                name: Some(String::from("crate2")),
                level: LevelFilter::Info,
            },
            Directive {
                name: Some(String::from("crate1::mod1")),
                level: LevelFilter::Warn,
            },
        ];
        assert!(!enabled(&logger, Level::Warn, "crate3"));
    }

    #[test]
    fn match_beginning() {
        let logger = vec![
            Directive {
                name: Some(String::from("crate2")),
                level: LevelFilter::Info,
            },
            Directive {
                name: Some(String::from("crate1::mod1")),
                level: LevelFilter::Warn,
            },
        ];
        assert!(enabled(&logger, Level::Info, "crate2::mod1"));
    }

    #[test]
    fn match_beginning_longest_match() {
        let logger = vec![
            Directive {
                name: Some(String::from("crate2")),
                level: LevelFilter::Info,
            },
            Directive {
                name: Some(String::from("crate2::mod")),
                level: LevelFilter::Debug,
            },
            Directive {
                name: Some(String::from("crate1::mod1")),
                level: LevelFilter::Warn,
            },
        ];
        assert!(enabled(&logger, Level::Debug, "crate2::mod1"));
        assert!(!enabled(&logger, Level::Debug, "crate2"));
    }

    #[test]
    fn match_default() {
        let logger = vec![
            Directive {
                name: None,
                level: LevelFilter::Info,
            },
            Directive {
                name: Some(String::from("crate1::mod1")),
                level: LevelFilter::Warn,
            },
        ];
        assert!(enabled(&logger, Level::Warn, "crate1::mod1"));
        assert!(enabled(&logger, Level::Info, "crate2::mod2"));
    }

    #[test]
    fn zero_level() {
        let logger = vec![
            Directive {
                name: None,
                level: LevelFilter::Info,
            },
            Directive {
                name: Some(String::from("crate1::mod1")),
                level: LevelFilter::Off,
            },
        ];
        assert!(!enabled(&logger, Level::Error, "crate1::mod1"));
        assert!(enabled(&logger, Level::Info, "crate2::mod2"));
    }

    #[test]
    fn parse_args_valid() {
        let mut dirs: VEC<Directive, 8> = VEC::new();
        parse_args(&mut dirs, "crate1::mod1=error,crate1::mod2,crate2=debug");

        assert_eq!(dirs.len(), 3);
        assert_eq!(dirs[0].name, Some(String::from("crate1::mod1")));
        assert_eq!(dirs[0].level, LevelFilter::Error);

        assert_eq!(dirs[1].name, Some(String::from("crate1::mod2")));
        assert_eq!(dirs[1].level, LevelFilter::max());

        assert_eq!(dirs[2].name, Some(String::from("crate2")));
        assert_eq!(dirs[2].level, LevelFilter::Debug);
    }

    #[test]
    fn parse_spec_invalid_crate() {
        // test parse_spec with multiple = in specification
        let mut dirs: VEC<Directive, 8> = VEC::new();
        parse_args(&mut dirs, "crate1::mod1=warn=info,crate2=debug");

        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].name, Some(String::from("crate2")));
        assert_eq!(dirs[0].level, LevelFilter::Debug);
    }

    #[test]
    fn parse_spec_invalid_level() {
        // test parse_spec with 'noNumber' as log level
        let mut dirs: VEC<Directive, 8> = VEC::new();
        parse_args(&mut dirs, "crate1::mod1=noNumber,crate2=debug");

        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].name, Some(String::from("crate2")));
        assert_eq!(dirs[0].level, LevelFilter::Debug);
    }

    #[test]
    fn parse_spec_string_level() {
        // test parse_spec with 'warn' as log level
        let mut dirs: VEC<Directive, 8> = VEC::new();
        parse_args(&mut dirs, "crate1::mod1=wrong,crate2=warn");

        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].name, Some(String::from("crate2")));
        assert_eq!(dirs[0].level, LevelFilter::Warn);
    }

    #[test]
    fn parse_spec_empty_level() {
        // test parse_spec with '' as log level\
        let mut dirs: VEC<Directive, 8> = VEC::new();
        parse_args(&mut dirs, "crate1::mod1=wrong,crate2=");

        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].name, Some(String::from("crate2")));
        assert_eq!(dirs[0].level, LevelFilter::max());
    }

    #[test]
    fn parse_spec_global() {
        // test parse_spec with no crate
        let mut dirs: VEC<Directive, 8> = VEC::new();
        parse_args(&mut dirs, "warn,crate2=debug");

        assert_eq!(dirs.len(), 2);
        assert_eq!(dirs[0].name, None);
        assert_eq!(dirs[0].level, LevelFilter::Warn);
        assert_eq!(dirs[1].name, Some(String::from("crate2")));
        assert_eq!(dirs[1].level, LevelFilter::Debug);
    }
}
