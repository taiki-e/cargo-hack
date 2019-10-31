use std::io::{self, Write};

use termcolor::{Color, ColorSpec, StandardStream, WriteColor};

use crate::cli::Coloring;

pub(crate) fn print_inner(
    coloring: Option<Coloring>,
    color: Option<Color>,
    kind: &str,
    write_msg: impl FnOnce(&mut StandardStream) -> io::Result<()>,
) {
    let mut stream = StandardStream::stderr(Coloring::color_choice(coloring));
    let _ = stream.set_color(ColorSpec::new().set_bold(true).set_fg(color));
    let _ = write!(stream, "{}", kind);
    let _ = stream.reset();
    let _ = writeln!(stream, ": ");
    let _ = write_msg(&mut stream);
}

macro_rules! error {
    ($coloring:expr, $($msg:expr),* $(,)?) => {{
        use std::io::Write;
        crate::term::print_inner($coloring, Some(termcolor::Color::Red), "error", |stream| writeln!(stream, $($msg),*));
    }};
}

macro_rules! warn {
    ($coloring:expr, $($msg:expr),* $(,)?) => {{
        use std::io::Write;
        crate::term::print_inner($coloring, Some(termcolor::Color::Yellow), "warning", |stream| writeln!(stream, $($msg),*));
    }};
}

macro_rules! info {
    ($coloring:expr, $($msg:expr),* $(,)?) => {{
        use std::io::Write;
        crate::term::print_inner($coloring, None, "info", |stream| writeln!(stream, $($msg),*));
    }};
}
