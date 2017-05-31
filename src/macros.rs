/// Obtaines a logger instance with the current module name passed
/// then passes the standard format! arguments to it.
#[macro_export]
macro_rules! slog{
	( $($arg:tt)* ) => ({
		use core::fmt::Write;
        use klogger::{Writer};
		let _ = write!(&mut Writer::get_module(module_path!()), $($arg)*);
	})
}

#[macro_export]
macro_rules! sprintln {
	( $($arg:tt)* ) => ({
		use core::fmt::Write;
        use klogger::{Writer};
		let _ = write!(&mut Writer::get(), $($arg)*);
	})
}
