use std::{
    env,
    io::Write,
    str::FromStr,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};

use anyhow::{format_err, Result};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

#[derive(PartialEq, Eq)]
#[repr(u8)]
enum Coloring {
    Auto = 0,
    Always,
    Never,
}

impl Coloring {
    const AUTO: u8 = Coloring::Auto as _;
    const ALWAYS: u8 = Coloring::Always as _;
    const NEVER: u8 = Coloring::Never as _;
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

static COLORING: AtomicU8 = AtomicU8::new(Coloring::AUTO);
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
    // Relaxed is fine because only the argument parsing step updates this value.
    COLORING.store(coloring as _, Ordering::Relaxed);
    Ok(())
}
fn coloring() -> ColorChoice {
    match COLORING.load(Ordering::Relaxed) {
        Coloring::AUTO => ColorChoice::Auto,
        Coloring::ALWAYS => ColorChoice::Always,
        Coloring::NEVER => ColorChoice::Never,
        _ => unreachable!(),
    }
}

static HAS_ERROR: AtomicBool = AtomicBool::new(false);
pub(crate) fn set_error() {
    HAS_ERROR.store(true, Ordering::SeqCst)
}
pub(crate) fn has_error() -> bool {
    HAS_ERROR.load(Ordering::SeqCst)
}

static VERBOSE: AtomicBool = AtomicBool::new(false);
pub(crate) struct VerboseGuard {
    prev: bool,
}
impl Drop for VerboseGuard {
    fn drop(&mut self) {
        set_verbose(self.prev);
    }
}
pub(crate) fn set_verbose(verbose: bool) {
    // TODO: CARGO_TERM_VERBOSE
    // https://doc.rust-lang.org/nightly/cargo/reference/config.html#termverbose
    VERBOSE.store(verbose, Ordering::Relaxed)
}
pub(crate) fn scoped_verbose(verbose: bool) -> VerboseGuard {
    // TODO: CARGO_TERM_VERBOSE
    // https://doc.rust-lang.org/nightly/cargo/reference/config.html#termverbose
    VerboseGuard { prev: VERBOSE.swap(verbose, Ordering::Relaxed) }
}
pub(crate) fn verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

pub(crate) fn print_status(status: &str, color: Option<Color>) -> StandardStream {
    let mut stream = StandardStream::stderr(coloring());
    let _ = stream.set_color(ColorSpec::new().set_bold(true).set_fg(color));
    let _ = write!(stream, "{}", status);
    let _ = stream.set_color(ColorSpec::new().set_bold(true));
    let _ = write!(stream, ":");
    let _ = stream.reset();
    let _ = write!(stream, " ");
    stream
}

macro_rules! error {
    ($($msg:expr),* $(,)?) => {{
        use std::io::Write;
        crate::term::set_error();
        let mut stream = crate::term::print_status("error", Some(termcolor::Color::Red));
        let _ = writeln!(stream, $($msg),*);
    }};
}

macro_rules! warn {
    ($($msg:expr),* $(,)?) => {{
        use std::io::Write;
        let mut stream = crate::term::print_status("warning", Some(termcolor::Color::Yellow));
        let _ = writeln!(stream, $($msg),*);
    }};
}

macro_rules! info {
    ($($msg:expr),* $(,)?) => {{
        use std::io::Write;
        let mut stream = crate::term::print_status("info", None);
        let _ = writeln!(stream, $($msg),*);
    }};
}
