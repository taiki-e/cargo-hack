use std::{
    borrow::Cow,
    collections::BTreeMap,
    env,
    ffi::OsString,
    io::{self, Write},
    path::PathBuf,
    process::Command,
};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::Args;

// Refs:
// * https://github.com/oli-obk/cargo_metadata
// * https://github.com/rust-lang/cargo/blob/0.40.0/src/cargo/ops/cargo_output_metadata.rs#L79
// * https://github.com/rust-lang/cargo/blob/0.40.0/src/cargo/core/package.rs#L57

#[derive(Debug, Deserialize)]
pub(crate) struct Metadata {
    /// A list of all crates referenced by this crate (and the crate itself)
    pub(crate) packages: Vec<Package>,
    /// Build directory
    pub(crate) target_directory: PathBuf,
    /// Workspace root
    pub(crate) workspace_root: PathBuf,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Package {
    /// Name as given in the `Cargo.toml`
    pub(crate) name: String,
    /// Features provided by the crate, mapped to the features required by that feature.
    pub(crate) features: BTreeMap<String, Vec<String>>,
    /// Path containing the `Cargo.toml`
    pub(crate) manifest_path: PathBuf,
}

impl Metadata {
    pub(crate) fn new(args: &Args) -> Result<Self> {
        let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
        let mut command = Command::new(cargo);
        command.args(&["metadata", "--no-deps", "--format-version=1"]);
        if let Some(manifest_path) = &args.manifest_path {
            command.arg("--manifest-path");
            command.arg(manifest_path);
        }

        let output = command.output().context("failed to run 'cargo metadata'")?;
        if !output.status.success() {
            let _ = io::stderr().write_all(&output.stderr);
            let code = output.status.code().unwrap_or(1);
            std::process::exit(code);
        }

        serde_json::from_slice(&output.stdout).context("failed to parse metadata")
    }
}

impl Package {
    pub(crate) fn name_verbose(&self, args: &Args) -> Cow<'_, str> {
        if args.verbose {
            Cow::Owned(format!(
                "{} ({})",
                self.name,
                self.manifest_path.parent().unwrap().display()
            ))
        } else {
            Cow::Borrowed(&self.name)
        }
    }
}
