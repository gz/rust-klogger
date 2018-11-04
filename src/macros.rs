#[macro_export]
macro_rules! sprintln {
	( $($arg:tt)* ) => ({
		use core::fmt::Write;
        use $crate::{Writer};
		let _ = write!(&mut Writer::get(), $($arg)*);
	})
}

#[macro_export]
macro_rules! sprint {
	( $($arg:tt)* ) => ({
		use core::fmt::Write;
        use $crate::{WriterNoDrop};
		let _ = write!(&mut WriterNoDrop::get(), $($arg)*);
	})
}
