/// Obtaines a logger instance with the current module name passed
/// then passes the standard format! arguments to it.
#[macro_export]
macro_rules! log{
	( $($arg:tt)* ) => ({
		use core::fmt::Write;
        use klogger::{Writer};
		let _ = write!(&mut Writer::get(module_path!()), $($arg)*);
	})
}