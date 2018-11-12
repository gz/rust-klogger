/// Write a string to the output channel.
pub unsafe fn puts(s: &str) {
    print!("{}", s);
}

pub unsafe fn putc(c: char) {
    print!("{}", c);
}
