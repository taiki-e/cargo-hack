use std::{
    env,
    ffi::{OsStr, OsString},
    fmt,
    path::Path,
    process::{Command, ExitStatus, Output},
    rc::Rc,
    str,
};

use anyhow::{Context as _, Result};

use crate::{Context, PackageId};

// Based on https://github.com/rust-lang/cargo/blob/0.47.0/src/cargo/util/process_builder.rs

/// A builder object for an external process, similar to `std::process::Command`.
#[derive(Clone)]
#[must_use]
pub(crate) struct ProcessBuilder<'a> {
    // $program $leading_args $propagated_leading_args $args $propagated_trailing_args
    /// The program to execute.
    program: Rc<OsStr>,
    /// A list of arguments to pass to the program (until '--').
    propagated_leading_args: &'a [&'a str],
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

    /// `true` to include full program path in display.
    display_program_path: bool,
    /// `true` to include manifest path in display.
    display_manifest_path: bool,
}

impl<'a> ProcessBuilder<'a> {
    /// Creates a new `ProcessBuilder`.
    pub(crate) fn new(program: impl AsRef<OsStr>) -> Self {
        Self {
            program: program.as_ref().into(),
            propagated_leading_args: &[],
            trailing_args: &[],
            leading_args: Vec::new(),
            args: Vec::new(),
            features: String::new(),
            display_program_path: false,
            display_manifest_path: false,
        }
    }

    pub(crate) fn with_args(&mut self, cx: &'a Context<'_>) -> &mut Self {
        self.propagated_leading_args = &cx.leading_args;
        self.trailing_args = cx.trailing_args;
        self.display_manifest_path = cx.verbose;
        self
    }

    pub(crate) fn append_features(&mut self, features: impl IntoIterator<Item = impl AsRef<str>>) {
        for feature in features {
            self.features.push_str(feature.as_ref());
            self.features.push(',');
        }
    }

    pub(crate) fn append_features_from_args(&mut self, cx: &Context<'_>, id: &PackageId) {
        if cx.ignore_unknown_features {
            self.append_features(cx.features.iter().filter(|&&f| {
                if cx.pkg_features(id).contains(f) {
                    true
                } else {
                    // ignored
                    info!("skipped applying unknown `{}` feature to {}", f, cx.packages(id).name);
                    false
                }
            }))
        } else if !cx.features.is_empty() {
            self.append_features(&cx.features);
        }
    }

    /// (chainable) Adds `arg` to the leading args list.
    pub(crate) fn leading_arg(&mut self, arg: impl Into<String>) -> &mut Self {
        self.leading_args.push(arg.into());
        self
    }

    /// (chainable) Adds `arg` to the args list.
    pub(crate) fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    /// (chainable) Adds multiple `args` to the args list.
    pub(crate) fn args(&mut self, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> &mut Self {
        self.args.extend(args.into_iter().map(|t| t.as_ref().to_os_string()));
        self
    }

    // /// (chainable) Replaces the args list with the given `args`.
    // pub(crate) fn args_replace(
    //     &mut self,
    //     args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    // ) -> &mut Self {
    //     self.args = args.into_iter().map(|t| t.as_ref().to_os_string()).collect();
    //     self
    // }

    /// (chainable) Enables full program path display.
    pub(crate) fn display_program_path(&mut self) -> &mut Self {
        self.display_program_path = true;
        self
    }

    /// (chainable) Enables manifest path display.
    pub(crate) fn display_manifest_path(&mut self) -> &mut Self {
        self.display_manifest_path = true;
        self
    }

    /// Enables all display* flags.
    fn display_all(&mut self) {
        self.display_program_path();
        self.display_manifest_path();
    }

    /// Gets the comma-separated features list
    fn get_features(&self) -> &str {
        // drop a trailing comma if it is not empty.
        &self.features[..self.features.len().saturating_sub(1)]
    }

    /// Executes the process, waiting for completion, and mapping non-success exit codes to an error.
    pub(crate) fn exec(&mut self) -> Result<()> {
        let mut cmd = self.build_command();

        let exit = cmd.status().with_context(|| {
            self.display_all();
            ProcessError::new(&format!("could not execute process {}", self), None, None)
        })?;

        if exit.success() {
            Ok(())
        } else {
            self.display_all();
            Err(ProcessError::new(
                &format!("process didn't exit successfully: {}", self),
                Some(exit),
                None,
            )
            .into())
        }
    }

    /// Executes the process, returning the stdio output, or an error if non-zero exit status.
    pub(crate) fn exec_with_output(&mut self) -> Result<Output> {
        let mut cmd = self.build_command();

        let output = cmd.output().with_context(|| {
            self.display_all();
            ProcessError::new(&format!("could not execute process {}", self), None, None)
        })?;

        if output.status.success() {
            Ok(output)
        } else {
            self.display_all();
            Err(ProcessError::new(
                &format!("process didn't exit successfully: {}", self),
                Some(output.status),
                Some(&output),
            )
            .into())
        }
    }

    /// Converts `ProcessBuilder` into a `std::process::Command`, and handles the jobserver, if
    /// present.
    fn build_command(&self) -> Command {
        let mut cmd = Command::new(&*self.program);

        cmd.args(&*self.leading_args);
        cmd.args(&*self.propagated_leading_args);
        cmd.args(&self.args);
        if !self.features.is_empty() {
            cmd.arg("--features");
            cmd.arg(self.get_features());
        }
        if !self.trailing_args.is_empty() {
            cmd.arg("--");
            cmd.args(&*self.trailing_args);
        }

        cmd
    }
}

impl fmt::Display for ProcessBuilder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`")?;

        if self.display_program_path {
            write!(f, "{}", self.program.to_string_lossy())?;
        } else {
            write!(f, "{}", Path::new(&*self.program).file_stem().unwrap().to_string_lossy())?;
        }

        for arg in &self.leading_args {
            write!(f, " {}", arg)?;
        }

        for arg in self.propagated_leading_args {
            write!(f, " {}", arg)?;
        }

        let mut args = self.args.iter();
        while let Some(arg) = args.next() {
            if arg == "--manifest-path" {
                let path = Path::new(args.next().unwrap());
                // Displaying `--manifest-path` is redundant.
                if self.display_manifest_path {
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

// Based on https://github.com/rust-lang/cargo/blob/0.47.0/src/cargo/util/errors.rs

#[derive(Debug)]
pub(crate) struct ProcessError {
    /// A detailed description to show to the user why the process failed.
    desc: String,
    /// The exit status of the process.
    ///
    /// This can be `None` if the process failed to launch (like process not found).
    exit: Option<ExitStatus>,
    /// The output from the process.
    ///
    /// This can be `None` if the process failed to launch, or the output was not captured.
    output: Option<Output>,
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
        let mut desc = format!("{} ({})", &msg, exit);

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

        Self { desc, exit: status, output: output.cloned() }
    }
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.desc, f)
    }
}

impl std::error::Error for ProcessError {}
