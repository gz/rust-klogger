#![cfg_attr(not(target_family = "unix"), no_std)]
#![crate_name = "klogger"]
#![crate_type = "lib"]

#[cfg(not(target_os = "none"))]
extern crate core;

use core::fmt;
use core::ops;

#[macro_use]
pub mod macros;

extern crate log;
extern crate termcodes;

#[cfg(target_arch = "x86_64")]
extern crate x86;

#[cfg(any(
    feature = "use_ioports",
    all(target_arch = "x86_64", target_os = "none")
))]
#[path = "arch/x86.rs"]
mod arch;

#[cfg(all(not(feature = "use_ioports"), target_family = "unix"))]
#[path = "arch/unix.rs"]
mod arch;

use log::{Level, Metadata, Record, SetLoggerError};
use termcodes::color;

/// One Mhz is that many Hz.
const MHZ_TO_HZ: u64 = 1000 * 1000;

/// One Khz is that many Hz.
const KHZ_TO_HZ: u64 = 1000;

/// One sec has that many ns.
const _NS_PER_SEC: u64 = 1_000_000_000u64;

/// Global lock to protect serial line from concurrent printing.
pub static SERIAL_LINE_MUTEX: spin::Mutex<bool> = spin::Mutex::new(false);

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
            let cur = unsafe { x86::time::rdtsc() };

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
}

impl log::Log for KLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
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

pub fn init(level: Level) -> Result<(), SetLoggerError> {
    let cpuid = x86::cpuid::CpuId::new();

    unsafe {
        LOGGER.has_tsc = cpuid
            .get_feature_info()
            .map_or(false, |finfo| finfo.has_tsc());
        LOGGER.has_invariant_tsc = cpuid
            .get_extended_function_info()
            .map_or(false, |efinfo| efinfo.has_invariant_tsc());

        if LOGGER.has_tsc {
            LOGGER.tsc_start = x86::time::rdtsc();
        }

        let tsc_frequency_hz: Option<u64> = cpuid.get_tsc_info().and_then(|tinfo| {
            if tinfo.nominal_frequency() != 0 {
                // If we have a crystal clock we can calculate the tsc frequency directly
                Some(tinfo.tsc_frequency())
            } else if tinfo.numerator() != 0 && tinfo.denominator() != 0 {
                // Skylake and Kabylake don't report the crystal clock frequency,
                // so we approximate with CPU base frequency:
                cpuid.get_processor_frequency_info().map(|pinfo| {
                    let cpu_base_freq_hz = pinfo.processor_base_frequency() as u64 * MHZ_TO_HZ;
                    let crystal_hz =
                        cpu_base_freq_hz * tinfo.denominator() as u64 / tinfo.numerator() as u64;
                    crystal_hz * tinfo.numerator() as u64 / tinfo.denominator() as u64
                })
            } else {
                // We couldn't figure out the TSC frequency
                None
            }
        });

        // Check if we run in a VM and the hypervisor can give us the TSC frequency
        let vmm_tsc_frequency_hz: Option<u64> = cpuid
            .get_hypervisor_info()
            .and_then(|hv| hv.tsc_frequency().map(|tsc_khz| tsc_khz as u64 * KHZ_TO_HZ));

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

        log::set_logger(&LOGGER).map(|()| log::set_max_level(level.to_level_filter()))
    }
}

pub fn putchar(c: char) {
    unsafe {
        arch::putc(c);
    }
}
