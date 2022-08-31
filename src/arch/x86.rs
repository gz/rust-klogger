use core::sync::atomic::AtomicU16;
use core::sync::atomic::Ordering;

extern crate x86;

use self::x86::io;

/// One Mhz is that many Hz.
const MHZ_TO_HZ: u64 = 1000 * 1000;

/// One Khz is that many Hz.
const KHZ_TO_HZ: u64 = 1000;

/// One sec has that many ns.
const _NS_PER_SEC: u64 = 1_000_000_000u64;

pub static SERIAL_PRINT_PORT: AtomicU16 = AtomicU16::new(0x3f8); /* default COM1 */

/// Write a string to the output channel.
pub unsafe fn puts(s: &str) {
    let port = SERIAL_PRINT_PORT.load(Ordering::Relaxed);
    for b in s.bytes() {
        putb(port, b);
    }
}

pub unsafe fn putc(c: char) {
    let port = SERIAL_PRINT_PORT.load(Ordering::Relaxed);
    putb(port, c as u8);
}

/// Write a single byte to the output channel.
unsafe fn putb(port: u16, b: u8) {
    // Wait for the serial FIFO to be ready
    while (io::inb(port + 5) & 0x20) == 0 {}
    io::outb(port, b);
}

pub fn set_output(port: u16) {
    SERIAL_PRINT_PORT.store(port, Ordering::Relaxed);
}

pub fn get_timestamp() -> u64 {
    unsafe { x86::time::rdtsc() }
}

pub fn has_tsc() -> bool {
    let cpuid = x86::cpuid::CpuId::new();
    cpuid
        .get_feature_info()
        .map_or(false, |finfo| finfo.has_tsc())
}

pub fn has_invariant_tsc() -> bool {
    let cpuid = x86::cpuid::CpuId::new();
    cpuid
        .get_advanced_power_mgmt_info()
        .map_or(false, |efinfo| efinfo.has_invariant_tsc())
}

pub fn get_tsc_frequency_hz() -> Option<u64> {
    let cpuid = x86::cpuid::CpuId::new();
    cpuid.get_tsc_info().and_then(|tinfo| {
        if tinfo.nominal_frequency() != 0 {
            // If we have a crystal clock we can calculate the tsc frequency directly
            tinfo.tsc_frequency()
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
    })
}

pub fn get_vmm_tsc_frequency_hz() -> Option<u64> {
    let cpuid = x86::cpuid::CpuId::new();
    cpuid
        .get_hypervisor_info()
        .and_then(|hv| hv.tsc_frequency().map(|tsc_khz| tsc_khz as u64 * KHZ_TO_HZ))
}
