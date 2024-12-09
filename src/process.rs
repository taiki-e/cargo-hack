// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    env,
    ffi::{OsStr, OsString},
    fmt,
    path::Path,
    process::{Command, ExitStatus, Output},
    rc::Rc,
    str,
};

use anyhow::{Context as _, Error, Result};

use crate::{term, Context, PackageId};

macro_rules! cmd {
    ($program:expr $(, $arg:expr)* $(,)?) => {{
        let mut _cmd = $crate::process::ProcessBuilder::new($program);
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
    program: Rc<OsStr>,
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
    pub(crate) strip_program_path: bool,
}

impl<'a> ProcessBuilder<'a> {
    /// Creates a new `ProcessBuilder`.
    pub(crate) fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into().into(),
            propagated_leading_args: &[],
            trailing_args: &[],
            leading_args: vec![],
            args: vec![],
            features: String::new(),
            strip_program_path: false,
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
        let package = cx.packages(id);
        let pkg_features = cx.pkg_features(id);
        let recursively_exclude_feature =
            cx.must_have_and_exclude_feature.as_ref().and_then(|s| pkg_features.get(s));

        self.append_features(cx.features.iter().filter(|&f| {
            if recursively_exclude_feature
                .is_some_and(|rf| rf.matches_recursive(f, &package.features))
            {
                info!(
                    "skipped applying `{f}` feature to {} because it would enable excluded feature `{}`",
                    package.name,
                    recursively_exclude_feature.unwrap().name()
                );
                false
            } else if cx.ignore_unknown_features && !pkg_features.contains(f) {
                info!("skipped applying unknown `{f}` feature to {}", package.name);
                false
            } else {
                true
            }
        }));
    }

    /// Gets the comma-separated features list
    fn get_features(&self) -> &str {
        // drop a trailing comma if it is not empty.
        &self.features[..self.features.len().saturating_sub(1)]
    }

    /// Executes a process, waiting for completion, and mapping non-zero exit
    /// status to an error.
    pub(crate) fn run(&self) -> Result<()> {
        let status = self.build().status().with_context(|| {
            process_error(format!("could not execute process {self:#}"), None, None)
        })?;
        if status.success() {
            Ok(())
        } else {
            Err(process_error(
                format!("process didn't exit successfully: {self:#}"),
                Some(status),
                None,
            ))
        }
    }

    /// Executes a process, captures its stdio output, returning the captured
    /// output, or an error if non-zero exit status.
    pub(crate) fn run_with_output(&self) -> Result<Output> {
        let output = self.build().output().with_context(|| {
            process_error(format!("could not execute process {self:#}"), None, None)
        })?;
        if output.status.success() {
            Ok(output)
        } else {
            Err(process_error(
                format!("process didn't exit successfully: {self:#}"),
                Some(output.status),
                Some(&output),
            ))
        }
    }

    /// Executes a process, captures its stdio output, returning the captured
    /// standard output as a `String`.
    pub(crate) fn read(&self) -> Result<String> {
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
        f.write_str("`")?;

        if !self.strip_program_path && (f.alternate() || term::verbose()) {
            f.write_str(&self.program.to_string_lossy())?;
        } else {
            f.write_str(&Path::new(&*self.program).file_stem().unwrap().to_string_lossy())?;
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
                        .and_then(|cwd| path.strip_prefix(cwd).ok())
                        .unwrap_or(path);
                    write!(f, " --manifest-path {}", path.display())?;
                }
            } else {
                write!(f, " {}", arg.to_string_lossy())?;
            }
        }

        if !self.features.is_empty() {
            write!(f, " --features {}", self.get_features())?;
        }

        if !self.trailing_args.is_empty() {
            f.write_str(" --")?;
            for arg in self.trailing_args {
                write!(f, " {arg}")?;
            }
        }

        f.write_str("`")
    }
}

// Based on https://github.com/rust-lang/cargo/blob/0.47.0/src/cargo/util/errors.rs
/// Creates a new process error.
///
/// `status` can be `None` if the process did not launch.
/// `output` can be `None` if the process did not launch, or output was not captured.
fn process_error(mut msg: String, status: Option<ExitStatus>, output: Option<&Output>) -> Error {
    match status {
        Some(s) => {
            msg.push_str(" (");
            msg.push_str(&s.to_string());
            msg.push(')');
        }
        None => msg.push_str(" (never executed)"),
    }

    if let Some(out) = output {
        match str::from_utf8(&out.stdout) {
            Ok(s) if !s.trim_start().is_empty() => {
                msg.push_str("\n--- stdout\n");
                msg.push_str(s);
            }
            Ok(_) | Err(_) => {}
        }
        match str::from_utf8(&out.stderr) {
            Ok(s) if !s.trim_start().is_empty() => {
                msg.push_str("\n--- stderr\n");
                msg.push_str(s);
            }
            Ok(_) | Err(_) => {}
        }
    }

    Error::msg(msg)
}
