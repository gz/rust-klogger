use crate::{KHZ_TO_HZ, MHZ_TO_HZ};

/// Write a string to the output channel.
pub unsafe fn puts(s: &str) {}

pub unsafe fn putc(c: char) {}

/// Write a single byte to the output channel.
unsafe fn putb(port: u16, b: u8) {}

pub fn set_output(port: u16) {}

pub fn get_timestamp() -> u64 {
    0
}

pub fn has_tsc() -> bool {
    false;
}

pub fn has_invariant_tsc() -> bool {
    false;
}

pub fn get_tsc_frequency_hz() -> Option<u64> {
    None
}

pub fn get_vmm_tsc_frequency_hz() -> Option<u64> {
    None
}
