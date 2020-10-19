use anyhow::bail;
use std::{
    io::{self, Write},
    sync::atomic::{AtomicU8, Ordering},
};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use crate::Result;

static COLORING: AtomicU8 = AtomicU8::new(0);

const AUTO: u8 = 0;
const ALWAYS: u8 = 1;
const NEVER: u8 = 2;

pub(crate) fn set_coloring(color: Option<&str>) -> Result<()> {
    let coloring = match color {
        Some("auto") | None => AUTO,
        Some("always") => ALWAYS,
        Some("never") => NEVER,
        Some(other) => bail!("must be auto, always, or never, but found `{}`", other),
    };
    COLORING.store(coloring, Ordering::Relaxed);
    Ok(())
}

fn coloring() -> ColorChoice {
    match COLORING.load(Ordering::Relaxed) {
        AUTO => ColorChoice::Auto,
        ALWAYS => ColorChoice::Always,
        NEVER => ColorChoice::Never,
        _ => unreachable!(),
    }
}

pub(crate) fn print_inner(
    color: Option<Color>,
    kind: &str,
    write_msg: impl FnOnce(&mut StandardStream) -> io::Result<()>,
) {
    let mut stream = StandardStream::stderr(coloring());
    let _ = stream.set_color(ColorSpec::new().set_bold(true).set_fg(color));
    let _ = write!(stream, "{}", kind);
    let _ = stream.reset();
    let _ = write!(stream, ": ");
    let _ = write_msg(&mut stream);
}

macro_rules! error {
    ($($msg:expr),* $(,)?) => {{
        use std::io::Write;
        crate::term::print_inner(Some(termcolor::Color::Red), "error", |stream| writeln!(stream, $($msg),*));
    }};
}

macro_rules! warn {
    ($($msg:expr),* $(,)?) => {{
        use std::io::Write;
        crate::term::print_inner(Some(termcolor::Color::Yellow), "warning", |stream| writeln!(stream, $($msg),*));
    }};
}

macro_rules! info {
    ($($msg:expr),* $(,)?) => {{
        use std::io::Write;
        crate::term::print_inner(None, "info", |stream| writeln!(stream, $($msg),*));
    }};
}
