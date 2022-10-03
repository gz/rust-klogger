use core::fmt::Write;

/// Write a string to the output channel.
pub unsafe fn puts(s: &str) {
    let mut uart = pl011_qemu::PL011::new(pl011_qemu::UART1::steal());
    uart.write_str(s).unwrap();
}

pub unsafe fn putc(c: char) {
    let mut uart = pl011_qemu::PL011::new(pl011_qemu::UART1::steal());
    uart.write_char(c).unwrap();
}

/// Write a single byte to the output channel.
unsafe fn putb(port: u16, b: u8) {
    let mut uart = pl011_qemu::PL011::new(pl011_qemu::UART1::steal());
    uart.write_byte(b);
}

pub fn set_output(port: u16) {}

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
