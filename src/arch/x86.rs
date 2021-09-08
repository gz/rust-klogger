use core::sync::atomic::AtomicU16;
use core::sync::atomic::Ordering;

use x86::io;

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
