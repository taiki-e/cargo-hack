// based on cargo-expand

use std::{
    ffi::{OsStr, OsString},
    fmt,
    path::Path,
    process::Command,
};

#[derive(Clone)]
pub(crate) struct Line {
    program: OsString,
    args: Vec<OsString>,
}

impl Line {
    pub(crate) fn new(program: impl AsRef<OsStr>) -> Self {
        Self { program: program.as_ref().to_os_string(), args: Vec::new() }
    }

    pub(crate) fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.args.push(arg.as_ref().to_owned());
        self
    }

    pub(crate) fn args<S: AsRef<OsStr>>(&mut self, args: impl IntoIterator<Item = S>) -> &mut Self {
        args.into_iter().for_each(|arg| self.args.push(arg.as_ref().to_owned()));
        self
    }

    // pub(crate) fn insert(&mut self, index: usize, arg: impl AsRef<OsStr>) {
    //     self.args.insert(index, arg.as_ref().to_owned());
    // }

    pub(crate) fn command(&self) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);
        cmd
    }
}

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Path::new(&self.program).file_stem().unwrap().to_string_lossy())?;
        self.args.iter().try_for_each(|arg| write!(f, " {}", arg.to_string_lossy()))
    }
}

impl IntoIterator for Line {
    type Item = OsString;
    type IntoIter = <Vec<OsString> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.args.into_iter()
    }
}
