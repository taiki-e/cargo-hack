use anyhow::Context as _;
use std::{
    env,
    ffi::{OsStr, OsString},
    fmt,
    path::Path,
    process::{Command, ExitStatus},
    rc::Rc,
    str,
};

use crate::{metadata::Package, Args, Result};

// Based on https://github.com/rust-lang/cargo/blob/0.44.0/src/cargo/util/process_builder.rs

/// A builder object for an external process, similar to `std::process::Command`.
#[derive(Clone)]
pub(crate) struct ProcessBuilder<'a> {
    /// The program to execute.
    program: Rc<OsStr>,
    /// A list of arguments to pass to the program (until '--').
    leading_args: &'a [&'a str],
    /// A list of arguments to pass to the program (after '--').
    trailing_args: &'a [String],

    /// A list of arguments to pass to the program (between `leading_args` and '--').
    args: Vec<OsString>,
    /// A comma-separated list of features.
    /// This list always has a trailing comma if it is not empty.
    // cargo less than Rust 1.38 cannot handle multiple '--features' flags, so it creates another String.
    features: String,

    /// Use verbose output.
    verbose: bool,
}

impl<'a> ProcessBuilder<'a> {
    /// Creates a new `ProcessBuilder`.
    pub(crate) fn new(program: impl Into<Rc<OsStr>>, verbose: bool) -> Self {
        Self {
            program: program.into(),
            leading_args: &[],
            trailing_args: &[],
            args: Vec::new(),
            features: String::new(),
            verbose,
        }
    }

    /// Creates a new `ProcessBuilder` from `Args`.
    pub(crate) fn from_args(program: impl Into<Rc<OsStr>>, args: &'a Args<'_>) -> Self {
        Self {
            program: program.into(),
            leading_args: &args.leading_args,
            trailing_args: args.trailing_args,
            args: Vec::new(),
            features: String::new(),
            verbose: args.verbose,
        }
    }

    pub(crate) fn append_features(&mut self, features: impl IntoIterator<Item = impl AsRef<str>>) {
        for feature in features {
            self.features.push_str(feature.as_ref());
            self.features.push(',');
        }
    }

    pub(crate) fn append_features_from_args(&mut self, args: &Args<'_>, package: &Package) {
        if args.ignore_unknown_features {
            self.append_features(args.features.iter().filter(|&&f| {
                if package.features.get(f).is_some()
                    || package.dependencies.iter().any(|dep| dep.as_feature() == Some(f))
                {
                    true
                } else {
                    // ignored
                    info!(
                        args.color,
                        "skipped applying unknown `{}` feature to {}", f, package.name,
                    );
                    false
                }
            }))
        } else if !args.features.is_empty() {
            self.append_features(&args.features);
        }
    }

    /// Adds `arg` to the args list.
    pub(crate) fn arg(&mut self, arg: impl AsRef<OsStr>) {
        self.args.push(arg.as_ref().to_os_string());
    }

    /// Gets the executable name.
    pub(crate) fn get_program(&self) -> &OsStr {
        &self.program
    }

    /// Gets the comma-separated features list
    fn get_features(&self) -> &str {
        // drop a trailing comma if it is not empty.
        &self.features[..self.features.len().saturating_sub(1)]
    }

    /// Executes the process, waiting for completion, and mapping non-success exit codes to an error.
    pub(crate) fn exec(&mut self) -> Result<()> {
        let mut command = self.build_command();
        let exit = command.status().with_context(|| {
            self.verbose = true;
            ProcessError::new(&format!("could not execute process {}", self), None)
        })?;

        if exit.success() {
            Ok(())
        } else {
            self.verbose = true;
            Err(ProcessError::new(
                &format!("process didn't exit successfully: {}", self),
                Some(exit),
            )
            .into())
        }
    }

    /// Converts `ProcessBuilder` into a `std::process::Command`, and handles the jobserver, if
    /// present.
    fn build_command(&self) -> Command {
        let mut command = Command::new(&*self.program);

        command.args(&*self.leading_args);
        command.args(&self.args);
        if !self.features.is_empty() {
            command.arg("--features");
            command.arg(self.get_features());
        }
        if !self.trailing_args.is_empty() {
            command.arg("--");
            command.args(&*self.trailing_args);
        }

        command
    }
}

impl fmt::Display for ProcessBuilder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`")?;

        write!(f, "{}", Path::new(&*self.program).file_stem().unwrap().to_string_lossy())?;

        for arg in &*self.leading_args {
            write!(f, " {}", arg)?;
        }

        let mut args = self.args.iter();
        while let Some(arg) = args.next() {
            if arg == "--manifest-path" {
                let path = Path::new(args.next().unwrap());
                // Displaying `--manifest-path` is redundant.
                if self.verbose {
                    let path = env::current_dir()
                        .ok()
                        .and_then(|cwd| path.strip_prefix(&cwd).ok())
                        .unwrap_or(path);

                    write!(f, " --manifest-path")?;
                    write!(f, " {}", path.display())?;
                }
            } else {
                write!(f, " {}", arg.to_string_lossy())?;
            }
        }

        if !self.features.is_empty() {
            write!(f, " --features {}", self.get_features())?;
        }

        if !self.trailing_args.is_empty() {
            write!(f, " --")?;
            for arg in &*self.trailing_args {
                write!(f, " {}", arg)?;
            }
        }

        write!(f, "`")
    }
}

// =============================================================================
// Process errors

// Based on https://github.com/rust-lang/cargo/blob/0.44.0/src/cargo/util/errors.rs

#[derive(Debug)]
pub(crate) struct ProcessError {
    /// A detailed description to show to the user why the process failed.
    desc: String,
    /// The exit status of the process.
    ///
    /// This can be `None` if the process failed to launch (like process not found).
    exit: Option<ExitStatus>,
}

impl ProcessError {
    /// Creates a new process error.
    ///
    /// `status` can be `None` if the process did not launch.
    fn new(msg: &str, status: Option<ExitStatus>) -> Self {
        let exit = match status {
            Some(s) => s.to_string(),
            None => "never executed".to_string(),
        };
        let desc = format!("{} ({})", &msg, exit);

        Self { desc, exit: status }
    }
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.desc, f)
    }
}

impl std::error::Error for ProcessError {}
