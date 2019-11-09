use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    fmt,
    path::Path,
    process::{Command, ExitStatus, Output},
    str,
};

use anyhow::{Context, Result};

// Based on https://github.com/rust-lang/cargo/blob/0.39.0/src/cargo/util/process_builder.rs

/// A builder object for an external process, similar to `std::process::Command`.
#[derive(Clone, Debug)]
pub(crate) struct ProcessBuilder {
    /// The program to execute.
    program: OsString,
    /// A list of arguments to pass to the program (until '--').
    args: Vec<OsString>,
    /// A list of arguments to pass to the program (after '--').
    args2: Vec<OsString>,
    /// Any environment variables that should be set for the program.
    env: HashMap<String, Option<OsString>>,
    /// The directory to run the program from.
    cwd: Option<OsString>,
}

impl ProcessBuilder {
    /// Creates a new `ProcessBuilder`.
    pub(crate) fn new(cmd: impl Into<OsString>) -> Self {
        Self {
            program: cmd.into(),
            args: Vec::new(),
            args2: Vec::new(),
            cwd: None,
            env: HashMap::new(),
        }
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

    /// (chainable) Adds multiple `args` to the args list.
    pub(crate) fn args(&mut self, args: &[impl AsRef<OsStr>]) -> &mut Self {
        self.args.extend(args.iter().map(|t| t.as_ref().to_os_string()));
        self
    }

    // /// (chainable) Replaces the args list with the given `args`.
    // pub(crate) fn args_replace(&mut self, args: &[impl AsRef<OsStr>]) -> &mut Self {
    //     self.args = args.iter().map(|t| t.as_ref().to_os_string()).collect();
    //     self
    // }

    // /// (chainable) Adds `arg` to the args2 list.
    // pub(crate) fn arg2(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
    //     self.args2.push(arg.as_ref().to_os_string());
    //     self
    // }

    /// (chainable) Adds multiple `args` to the args2 list.
    pub(crate) fn args2(&mut self, args: &[impl AsRef<OsStr>]) -> &mut Self {
        self.args2.extend(args.iter().map(|t| t.as_ref().to_os_string()));
        self
    }

    // /// (chainable) Replaces the args2 list with the given `args`.
    // pub(crate) fn args2_replace(&mut self, args: &[impl AsRef<OsStr>]) -> &mut Self {
    //     self.args2 = args.iter().map(|t| t.as_ref().to_os_string()).collect();
    //     self
    // }

    // /// (chainable) Sets the current working directory of the process.
    // pub(crate) fn cwd(&mut self, path: impl AsRef<OsStr>) -> &mut Self {
    //     self.cwd = Some(path.as_ref().to_os_string());
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

    /// Gets the current working directory for the process.
    pub(crate) fn get_cwd(&self) -> Option<&Path> {
        self.cwd.as_ref().map(Path::new)
    }

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
        let mut command = Command::new(&self.program);
        if let Some(cwd) = self.get_cwd() {
            command.current_dir(cwd);
        }
        command.args(&self.args);
        command.arg("--");
        command.args(&self.args2);
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

        write!(f, "{}", Path::new(&self.program).file_stem().unwrap().to_string_lossy())?;

        for arg in &self.args {
            write!(f, " {}", arg.to_string_lossy())?;
        }
        for arg in &self.args2 {
            write!(f, " {}", arg.to_string_lossy())?;
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
