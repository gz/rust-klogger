#![no_std]
#![crate_name = "klogger"]
#![crate_type = "lib"]

use core::fmt;
pub mod macros;

#[cfg(target_arch = "x86_64")]
extern crate x86;

#[cfg(target_arch = "x86_64")]
#[path = "arch/x86.rs"]
mod arch;

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
