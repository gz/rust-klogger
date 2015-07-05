#![feature(no_std)]
#![feature(core)]
#![feature(core_prelude)]
#![feature(core_str_ext)]
#![no_std]

#![crate_name = "klogger"]
#![crate_type = "lib"]

#[macro_use]
extern crate core;

use core::prelude::*;
use core::atomic;
use core::fmt;

pub mod macros;

#[cfg(test)]
#[macro_use]
extern crate std;

#[cfg(target_arch="x86_64")]
#[macro_use]
extern crate x86;

#[cfg(target_arch="x86_64")]
#[path="arch/x86.rs"]
mod arch;

pub struct Writer;

impl Writer {
    /// Obtain a logger for the specified module.
    pub fn get(module: &str) -> Writer {
        use core::fmt::Write;
        let mut ret = Writer;
        write!(&mut ret, "[{}] ", module);
        ret
    }
}

impl ::core::ops::Drop for Writer {
    /// Release the logger.
    fn drop(&mut self) {
        use core::fmt::Write;
        write!(self, "\n");
    }
}

impl fmt::Write for Writer {
    /// Write stuff to serial out.
    fn write_str(&mut self, s: &str) -> fmt::Result {
        unsafe { arch::puts(s); }
        Ok(())
    }
}
