#![cfg_attr(not(target_family = "unix"), no_std)]
#![crate_name = "klogger"]
#![crate_type = "lib"]

#[cfg(not(target_family = "unix"))]
mod std {
    pub use core::fmt;
    pub use core::ops;
}

use std::fmt;
use std::ops;

#[macro_use]
pub mod macros;

extern crate log;
extern crate termcodes;

#[macro_use]
extern crate raw_cpuid;

#[cfg(target_arch = "x86_64")]
extern crate x86;

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
#[path = "arch/x86.rs"]
mod arch;

#[cfg(target_family = "unix")]
#[path = "arch/unix.rs"]
mod arch;

use log::{Level, Metadata, Record, SetLoggerError};
use termcodes::color;

static mut LOGGER: KLogger = KLogger {
    has_tsc: false,
    has_invariant_tsc: false,

    tsc_start: 0,
    tsc_frequency: 2_000_000_000,
};

pub struct Writer;

impl Writer {
    /// Obtain a logger for the specified module.
    pub fn get_module(module: &str) -> Writer {
        use std::fmt::Write;
        let mut ret = Writer;
        write!(&mut ret, "[{}] ", module).expect("Writer");
        ret
    }

    pub fn get() -> Writer {
        Writer
    }
}

impl ops::Drop for Writer {
    /// Release the logger.
    fn drop(&mut self) {
        use std::fmt::Write;
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

#[derive(Debug)]
struct KLogger {
    has_tsc: bool,
    has_invariant_tsc: bool,
    tsc_start: u64,
    /// Frequency in Hz
    tsc_frequency: u64,
}

impl KLogger {
    /// Time in nano seconds since KLogger init.
    fn elapsed_ns(&self) -> u64 {
        if self.has_tsc {
            let cur = unsafe { x86::time::rdtsc() };
            let elapsed = (cur - self.tsc_start) as f64;
            (elapsed / (self.tsc_frequency as f64 / 1_000_000_000.0)) as u64
        } else {
            0
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
                "{}{:>10}{} [{}{:5}{}] - {}: {}{}{}",
                color::Fg(color::LightYellow),
                self.elapsed_ns(),
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
    let cpuid = raw_cpuid::CpuId::new();

    unsafe {
        (&mut LOGGER).has_tsc = cpuid
            .get_feature_info()
            .map_or(false, |finfo| finfo.has_tsc());
        (&mut LOGGER).has_invariant_tsc = cpuid
            .get_extended_function_info()
            .map_or(false, |efinfo| efinfo.has_invariant_tsc());
        if LOGGER.has_tsc {
            (&mut LOGGER).tsc_start = x86::time::rdtsc();
        }
        let hypervisor_base = cpuid!(0x40000000, 0);
        let is_kvm = hypervisor_base.eax == 0x40000010
            && hypervisor_base.ebx == 0x4b4d564b
            && hypervisor_base.ecx == 0x564b4d56
            && hypervisor_base.edx == 0x4d;

        if cpuid.get_tsc_info().is_some() {
            // Nominal TSC frequency = ( CPUID.15H.ECX[31:0] * CPUID.15H.EBX[31:0] ) ÷ CPUID.15H.EAX[31:0]
            (&mut LOGGER).tsc_frequency = cpuid
                .get_tsc_info()
                .map_or(2_000_000_000, |tinfo| tinfo.tsc_frequency());
        } else if cpuid.get_processor_frequency_info().is_some() {
            (&mut LOGGER).tsc_frequency = cpuid
                .get_processor_frequency_info()
                .map_or(2_000_000_000, |pinfo| {
                    pinfo.processor_max_frequency() as u64 * 1000000
                });
        } else if is_kvm {
            // vm aware tsc frequency retrieval: https://lwn.net/Articles/301888/
            // # EAX: (Virtual) TSC frequency in kHz.
            // # EBX: (Virtual) Bus (local apic timer) frequency in kHz.
            // # ECX, EDX: RESERVED (Per above, reserved fields are set to zero).
            let virt_tinfo = cpuid!(0x40000010, 0);
            (&mut LOGGER).tsc_frequency = virt_tinfo.eax as u64 * 1000;
        } else {
            (&mut LOGGER).tsc_frequency = 2_000_000_000;
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
