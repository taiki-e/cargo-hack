use std::{
    env,
    io::Write,
    str::FromStr,
    sync::atomic::{AtomicU8, Ordering::Relaxed},
};

use anyhow::{format_err, Result};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

static COLORING: AtomicU8 = AtomicU8::new(AUTO);

const AUTO: u8 = Coloring::Auto as _;
const ALWAYS: u8 = Coloring::Always as _;
const NEVER: u8 = Coloring::Never as _;

#[derive(PartialEq, Eq)]
#[repr(u8)]
enum Coloring {
    Auto = 0,
    Always,
    Never,
}

impl FromStr for Coloring {
    type Err = String;

    fn from_str(color: &str) -> Result<Self, Self::Err> {
        match color {
            "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            other => Err(format!("must be auto, always, or never, but found `{}`", other)),
        }
    }
}

pub(crate) fn set_coloring(color: Option<&str>) -> Result<()> {
    let mut coloring = match color {
        Some(color) => color.parse().map_err(|e| format_err!("argument for --color {}", e))?,
        // https://doc.rust-lang.org/nightly/cargo/reference/config.html#termcolor
        None => match env::var_os("CARGO_TERM_COLOR") {
            Some(color) => color
                .to_string_lossy()
                .parse()
                .map_err(|e| format_err!("CARGO_TERM_COLOR {}", e))?,
            None => Coloring::Auto,
        },
    };
    if coloring == Coloring::Auto && !atty::is(atty::Stream::Stderr) {
        coloring = Coloring::Never;
    }
    COLORING.store(coloring as _, Relaxed);
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
        let mut stream = crate::term::print_inner(Some(termcolor::Color::Red), "error");
        let _ = writeln!(stream, $($msg),*);
    }};
}

macro_rules! warn {
    ($($msg:expr),* $(,)?) => {{
        use std::io::Write;
        let mut stream = crate::term::print_inner(Some(termcolor::Color::Yellow), "warning");
        let _ = writeln!(stream, $($msg),*);
    }};
}

macro_rules! info {
    ($($msg:expr),* $(,)?) => {{
        use std::io::Write;
        let mut stream = crate::term::print_inner(None, "info");
        let _ = writeln!(stream, $($msg),*);
    }};
}
