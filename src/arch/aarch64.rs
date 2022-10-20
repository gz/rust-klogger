use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

pub static SERIAL_PRINT_PORT: AtomicU64 = AtomicU64::new(0xffff_0000_0900_0000);

pl011_drv::create_uart!(
    /// Hardware Singleton for UART1 
    struct UartAarch64,
    UartAarch64_TAKEN, 0xffff_0000_0900_0000);

/// Write a string to the output channel.
pub unsafe fn puts(s: &str) {
    let mut uart = pl011_drv::PL011::new(UartAarch64::steal());
    uart.write_str(s).unwrap();
}

pub unsafe fn putc(c: char) {
    let mut uart = pl011_drv::PL011::new(UartAarch64::steal());
    uart.write_char(c).unwrap();
}

/// Write a single byte to the output channel.
unsafe fn putb(port: u16, b: u8) {
    let mut uart = pl011_drv::PL011::new(UartAarch64::steal());
    uart.write_byte(b);
}

pub fn set_output(port: u64) {}

pub fn get_timestamp() -> u64 {
    0
}

pub fn has_tsc() -> bool {
    false
}

pub fn has_invariant_tsc() -> bool {
    false
}

pub fn get_tsc_frequency_hz() -> Option<u64> {
    None
}

pub fn get_vmm_tsc_frequency_hz() -> Option<u64> {
    None
}
