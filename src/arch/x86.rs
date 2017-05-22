use x86::shared::io;

/// Write a string to the output channel.
pub unsafe fn puts(s: &str) {
    for b in s.bytes() {
        // TODO: hard-coded serial line 0.
        putb(0x3f8, b);
    }
}

/// Write a single byte to the output channel.
pub unsafe fn putb(port: u16, b: u8) {
    // Wait for the serial FIFO to be ready
    while (io::inb(port + 5) & 0x20) == 0 {}
    io::outb(port, b);
}
