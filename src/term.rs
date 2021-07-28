use std::{
    env,
    io::Write,
    sync::atomic::{AtomicU8, Ordering::Relaxed},
};

use anyhow::{bail, Result};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

static COLORING: AtomicU8 = AtomicU8::new(AUTO);

const AUTO: u8 = 0;
const ALWAYS: u8 = 1;
const NEVER: u8 = 2;

pub(crate) fn set_coloring(color: Option<&str>) -> Result<()> {
    // https://doc.rust-lang.org/cargo/reference/config.html#termcolor
    let cargo_term_color = if color.is_none() { env::var("CARGO_TERM_COLOR").ok() } else { None };
    let coloring = match color.or_else(|| cargo_term_color.as_ref().map(|s| &**s)) {
        Some("auto") | None => AUTO,
        Some("always") => ALWAYS,
        Some("never") => NEVER,
        Some(other) => bail!("must be auto, always, or never, but found `{}`", other),
    };
    COLORING.store(coloring, Relaxed);
    Ok(())
}

fn coloring() -> ColorChoice {
    match COLORING.load(Relaxed) {
        AUTO => ColorChoice::Auto,
        ALWAYS => ColorChoice::Always,
        NEVER => ColorChoice::Never,
        _ => unreachable!(),
    }
}

pub(crate) fn print_inner(color: Option<Color>, kind: &str) -> StandardStream {
    let mut stream = StandardStream::stderr(coloring());
    let _ = stream.set_color(ColorSpec::new().set_bold(true).set_fg(color));
    let _ = write!(stream, "{}", kind);
    let _ = stream.reset();
    let _ = write!(stream, ": ");
    stream
}

macro_rules! error {
    ($($msg:expr),* $(,)?) => {{
        use std::io::Write;
        let mut stream = crate::term::print_inner(
            Some(termcolor::Color::Red),
            "error",
        );
        let _ = writeln!(stream, $($msg),*);
    }};
}

macro_rules! warn {
    ($($msg:expr),* $(,)?) => {{
        use std::io::Write;
        let mut stream = crate::term::print_inner(
            Some(termcolor::Color::Yellow),
            "warning",
        );
        let _ = writeln!(stream, $($msg),*);
    }};
}

macro_rules! info {
    ($($msg:expr),* $(,)?) => {{
        use std::io::Write;
        let mut stream = crate::term::print_inner(
            None,
            "info",
        );
        let _ = writeln!(stream, $($msg),*);
    }};
}
