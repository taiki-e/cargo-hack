use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    fmt,
    path::Path,
    process::{Command, ExitStatus, Output},
    rc::Rc,
    str,
};

use anyhow::{Context, Result};

use crate::{Args, Package};

// Based on https://github.com/rust-lang/cargo/blob/0.39.0/src/cargo/util/process_builder.rs

/// A builder object for an external process, similar to `std::process::Command`.
#[derive(Clone, Debug)]
pub(crate) struct ProcessBuilder {
    /// The program to execute.
    program: Rc<OsString>,
    /// A list of arguments to pass to the program (until '--').
    leading_args: Rc<[String]>,
    /// A list of arguments to pass to the program (after '--').
    trailing_args: Rc<[String]>,
    /// A list of arguments to pass to the program (between `leading_args` and '--').
    args: Vec<OsString>,
    // cargo less than Rust 1.38 cannot handle multiple '--features' flags, so it creates another String.
    features: String,

    /// Any environment variables that should be set for the program.
    env: HashMap<String, Option<OsString>>,
    /// Use verbose output.
    verbose: bool,
}

impl ProcessBuilder {
    /// Creates a new `ProcessBuilder`.
    pub(crate) fn new(program: OsString) -> Self {
        Self {
            program: Rc::new(program),
            leading_args: Rc::from(&[][..]),
            trailing_args: Rc::from(&[][..]),
            args: Vec::new(),
            features: String::new(),
            env: HashMap::new(),
            verbose: false,
        }
    }

    /// Creates a new `ProcessBuilder` from `Args`.
    pub(crate) fn from_args(program: OsString, args: &Args) -> Self {
        Self {
            program: Rc::new(program),
            leading_args: args.leading_args.clone(),
            trailing_args: args.trailing_args.clone(),
            args: Vec::new(),
            features: String::new(),
            env: HashMap::new(),
            verbose: args.verbose,
        }
    }

    pub(crate) fn append_features(&mut self, features: impl IntoIterator<Item = impl AsRef<str>>) {
        for feature in features {
            self.features.push_str(feature.as_ref());
            self.features.push(',');
        }
    }

    /// (chainable) Adds `--features` flag to the args list.
    pub(crate) fn features(&mut self, args: &Args, package: &Package) -> &mut Self {
        if args.ignore_unknown_features {
            self.append_features(args.features.iter().filter(|f| {
                if package.features.get(*f).is_some() {
                    true
                } else {
                    // ignored
                    info!(
                        args.color,
                        "skipped applying unknown `{}` feature to {}", f, package.name
                    );
                    false
                }
            }))
        } else if !args.features.is_empty() {
            self.append_features(&args.features);
        }
        self
    }

    // /// (chainable) Sets the executable for the process.
    // pub(crate) fn program(&mut self, program: impl AsRef<OsStr>) -> &mut Self {
    //     self.program = program.as_ref().to_os_string();
    //     self
    // }

    /// (chainable) Adds `arg` to the args list.
    pub(crate) fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    // /// (chainable) Adds multiple `args` to the args list.
    // pub(crate) fn args(&mut self, args: &[impl AsRef<OsStr>]) -> &mut Self {
    //     self.args.extend(args.iter().map(|t| t.as_ref().to_os_string()));
    //     self
    // }

    // /// (chainable) Replaces the args list with the given `args`.
    // pub(crate) fn args_replace(&mut self, args: &[impl AsRef<OsStr>]) -> &mut Self {
    //     self.args = args.iter().map(|t| t.as_ref().to_os_string()).collect();
    //     self
    // }

    // /// (chainable) Sets an environment variable for the process.
    // pub(crate) fn env(&mut self, key: &str, val: impl AsRef<OsStr>) -> &mut Self {
    //     self.env.insert(key.to_string(), Some(val.as_ref().to_os_string()));
    //     self
    // }

    // /// (chainable) Unsets an environment variable for the process.
    // pub(crate) fn env_remove(&mut self, key: &str) -> &mut Self {
    //     self.env.insert(key.to_string(), None);
    //     self
    // }

    // /// Gets the executable name.
    // pub(crate) fn get_program(&self) -> &OsString {
    //     &self.program
    // }

    // /// Gets the program arguments.
    // pub(crate) fn get_args(&self) -> &[OsString] {
    //     &self.args
    // }

    // /// Gets an environment variable as the process will see it (will inherit from environment
    // /// unless explicitally unset).
    // pub(crate) fn get_env(&self, var: &str) -> Option<OsString> {
    //     self.env.get(var).cloned().or_else(|| Some(env::var_os(var))).and_then(|s| s)
    // }

    // /// Gets all environment variables explicitly set or unset for the process (not inherited
    // /// vars).
    // pub(crate) fn get_envs(&self) -> &HashMap<String, Option<OsString>> {
    //     &self.env
    // }

    /// Runs the process, waiting for completion, and mapping non-success exit codes to an error.
    pub(crate) fn exec(&self) -> Result<()> {
        let mut command = self.build_command();
        let exit = command.status().with_context(|| {
            ProcessError::new(&format!("could not execute process {}", self), None, None)
        })?;

        if exit.success() {
            Ok(())
        } else {
            Err(ProcessError::new(
                &format!("process didn't exit successfully: {}", self),
                Some(exit),
                None,
            )
            .into())
        }
    }

    // /// Executes the process, returning the stdio output, or an error if non-zero exit status.
    // pub(crate) fn exec_with_output(&self) -> Result<Output> {
    //     let mut command = self.build_command();

    //     let output = command.output().with_context(|| {
    //         ProcessError::new(&format!("could not execute process {}", self), None, None)
    //     })?;

    //     if output.status.success() {
    //         Ok(output)
    //     } else {
    //         Err(ProcessError::new(
    //             &format!("process didn't exit successfully: {}", self),
    //             Some(output.status),
    //             Some(&output),
    //         )
    //         .into())
    //     }
    // }

    /// Converts `ProcessBuilder` into a `std::process::Command`, and handles the jobserver, if
    /// present.
    fn build_command(&self) -> Command {
        let mut command = Command::new(&*self.program);

        command.args(&*self.leading_args);
        command.args(&self.args);
        if !self.features.is_empty() {
            command.arg("--features");
            command.arg(&self.features[..self.features.len() - 1]);
        }
        if !self.trailing_args.is_empty() {
            command.arg("--");
            command.args(&*self.trailing_args);
        }

        for (k, v) in &self.env {
            match v {
                Some(v) => {
                    command.env(k, v);
                }
                None => {
                    command.env_remove(k);
                }
            }
        }
        command
    }
}

impl fmt::Display for ProcessBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`")?;

        write!(f, "{}", Path::new(&*self.program).file_stem().unwrap().to_string_lossy())?;

        for arg in &*self.leading_args {
            write!(f, " {}", arg)?;
        }

        if self.verbose {
            for arg in &self.args {
                write!(f, " {}", arg.to_string_lossy())?;
            }
        } else {
            let mut args = self.args.iter();
            while let Some(arg) = args.next() {
                // Displaying `--manifest-path` is redundant.
                if arg == "--manifest-path" {
                    let _ = args.next();
                    continue;
                }
                write!(f, " {}", arg.to_string_lossy())?;
            }
        }

        if !self.features.is_empty() {
            write!(f, " --features {}", &self.features[..self.features.len() - 1])?;
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

// Based on https://github.com/rust-lang/cargo/blob/0.39.0/src/cargo/util/errors.rs

#[derive(Debug)]
pub(crate) struct ProcessError {
    desc: String,
    exit: Option<ExitStatus>,
    output: Option<Output>,
}

impl ProcessError {
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
