use std::{
    env,
    ffi::{OsStr, OsString},
    fmt,
    path::Path,
    process::{Command, ExitStatus, Output},
    str,
    sync::Arc,
};

use anyhow::{Context as _, Result};

use crate::{term, Context, PackageId};

macro_rules! cmd {
    ($program:expr $(, $arg:expr)* $(,)?) => {{
        let mut _cmd = crate::process::ProcessBuilder::new($program);
        $(
            _cmd.arg($arg);
        )*
        _cmd
    }};
}

// A builder for an external process, inspired by https://github.com/rust-lang/cargo/blob/0.47.0/src/cargo/util/process_builder.rs
//
// The fields will be expanded in the following order:
//   <program> <leading_args> <propagated_leading_args> <arg> [--features <features>] [ -- <propagated_trailing_args> ]
#[derive(Clone)]
#[must_use]
pub(crate) struct ProcessBuilder<'a> {
    /// The program to execute.
    program: Arc<OsStr>,
    /// A list of arguments to pass to the program (until '--').
    propagated_leading_args: &'a [String],
    /// A list of arguments to pass to the program (after '--').
    trailing_args: &'a [String],

    /// A list of arguments to pass to the program (between `program` and 'propagated_leading_args').
    leading_args: Vec<String>,
    /// A list of arguments to pass to the program (between `propagated_leading_args` and '--').
    args: Vec<OsString>,
    /// A comma-separated list of features.
    /// This list always has a trailing comma if it is not empty.
    // cargo less than Rust 1.38 cannot handle multiple '--features' flags, so it creates another String.
    features: String,
}

impl<'a> ProcessBuilder<'a> {
    /// Creates a new `ProcessBuilder`.
    pub(crate) fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into().into(),
            propagated_leading_args: &[],
            trailing_args: &[],
            leading_args: Vec::new(),
            args: Vec::new(),
            features: String::new(),
        }
    }

    /// Adds an argument to pass to the program.
    pub(crate) fn arg(&mut self, arg: impl Into<OsString>) -> &mut Self {
        self.args.push(arg.into());
        self
    }

    /// Adds multiple arguments to pass to the program.
    pub(crate) fn args(
        &mut self,
        args: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> &mut Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Adds an argument to the leading arguments list.
    pub(crate) fn leading_arg(&mut self, arg: impl Into<String>) -> &mut Self {
        self.leading_args.push(arg.into());
        self
    }

    pub(crate) fn apply_context(&mut self, cx: &'a Context) -> &mut Self {
        self.propagated_leading_args = &cx.leading_args;
        self.trailing_args = &cx.trailing_args;
        self
    }

    pub(crate) fn append_features(&mut self, features: impl IntoIterator<Item = impl AsRef<str>>) {
        for feature in features {
            self.features.push_str(feature.as_ref());
            self.features.push(',');
        }
    }

    pub(crate) fn append_features_from_args(&mut self, cx: &Context, id: &PackageId) {
        if cx.ignore_unknown_features {
            self.append_features(cx.features.iter().filter(|&f| {
                if cx.pkg_features(id).contains(f) {
                    true
                } else {
                    // ignored
                    info!("skipped applying unknown `{f}` feature to {}", cx.packages(id).name);
                    false
                }
            }));
        } else if !cx.features.is_empty() {
            self.append_features(&cx.features);
        }
    }

    /// Gets the comma-separated features list
    fn get_features(&self) -> &str {
        // drop a trailing comma if it is not empty.
        &self.features[..self.features.len().saturating_sub(1)]
    }

    /// Executes a process, waiting for completion, and mapping non-zero exit
    /// status to an error.
    pub(crate) fn run(&mut self) -> Result<()> {
        let status = self.build().status().with_context(|| {
            ProcessError::new(&format!("could not execute process {self:#}"), None, None)
        })?;
        if status.success() {
            Ok(())
        } else {
            Err(ProcessError::new(
                &format!("process didn't exit successfully: {self:#}"),
                Some(status),
                None,
            )
            .into())
        }
    }

    /// Functionally similar to `run(&mut self) -> Result<()>` but with support to provide
    /// key-value pair of environment variable
    pub(crate) fn run_with_env<'b>(&mut self, env: (&'b str, &'b str)) -> Result<()> {
        let status = self.build().env(env.0, env.1).status().with_context(|| {
            ProcessError::new(&format!("could not execute process {self:#}"), None, None)
        })?;
        if status.success() {
            Ok(())
        } else {
            Err(ProcessError::new(
                &format!("process didn't exit successfully: {self:#}"),
                Some(status),
                None,
            )
            .into())
        }
    }

    /// Executes a process, captures its stdio output, returning the captured
    /// output, or an error if non-zero exit status.
    pub(crate) fn run_with_output(&mut self) -> Result<Output> {
        let output = self.build().output().with_context(|| {
            ProcessError::new(&format!("could not execute process {self:#}"), None, None)
        })?;
        if output.status.success() {
            Ok(output)
        } else {
            Err(ProcessError::new(
                &format!("process didn't exit successfully: {self:#}"),
                Some(output.status),
                Some(&output),
            )
            .into())
        }
    }

    /// Executes a process, captures its stdio output, returning the captured
    /// standard output as a `String`.
    pub(crate) fn read(&mut self) -> Result<String> {
        let mut output = String::from_utf8(self.run_with_output()?.stdout)
            .with_context(|| format!("failed to parse output from {self:#}"))?;
        while output.ends_with('\n') || output.ends_with('\r') {
            output.pop();
        }
        Ok(output)
    }

    fn build(&self) -> Command {
        let mut cmd = Command::new(&*self.program);

        cmd.args(&*self.leading_args);
        cmd.args(self.propagated_leading_args);
        cmd.args(&self.args);
        if !self.features.is_empty() {
            cmd.arg("--features");
            cmd.arg(self.get_features());
        }
        if !self.trailing_args.is_empty() {
            cmd.arg("--");
            cmd.args(self.trailing_args);
        }

        cmd
    }
}

impl fmt::Display for ProcessBuilder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`")?;

        if f.alternate() || term::verbose() {
            write!(f, "{}", self.program.to_string_lossy())?;
        } else {
            write!(f, "{}", Path::new(&*self.program).file_stem().unwrap().to_string_lossy())?;
        }

        for arg in &self.leading_args {
            write!(f, " {arg}")?;
        }

        for arg in self.propagated_leading_args {
            write!(f, " {arg}")?;
        }

        let mut args = self.args.iter();
        while let Some(arg) = args.next() {
            if arg == "--manifest-path" {
                let path = Path::new(args.next().unwrap());
                // Displaying `--manifest-path` is redundant.
                if f.alternate() || term::verbose() {
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
            for arg in self.trailing_args {
                write!(f, " {arg}")?;
            }
        }

        write!(f, "`")
    }
}

// Based on https://github.com/rust-lang/cargo/blob/0.47.0/src/cargo/util/errors.rs
#[derive(Debug)]
struct ProcessError {
    /// A detailed description to show to the user why the process failed.
    desc: String,
}

impl ProcessError {
    /// Creates a new process error.
    ///
    /// `status` can be `None` if the process did not launch.
    /// `output` can be `None` if the process did not launch, or output was not captured.
    fn new(msg: &str, status: Option<ExitStatus>, output: Option<&Output>) -> Self {
        let exit = match status {
            Some(s) => s.to_string(),
            None => "never executed".to_string(),
        };
        let mut desc = format!("{} ({exit})", &msg);

        if let Some(out) = output {
            match str::from_utf8(&out.stdout) {
                Ok(s) if !s.trim().is_empty() => {
                    desc.push_str("\n--- stdout\n");
                    desc.push_str(s);
                }
                Ok(_) | Err(_) => {}
            }
            match str::from_utf8(&out.stderr) {
                Ok(s) if !s.trim().is_empty() => {
                    desc.push_str("\n--- stderr\n");
                    desc.push_str(s);
                }
                Ok(_) | Err(_) => {}
            }
        }

        Self { desc }
    }
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.desc, f)
    }
}

impl std::error::Error for ProcessError {}
