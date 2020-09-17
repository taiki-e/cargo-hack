use anyhow::Context;
use serde_derive::Deserialize;
use std::{
    borrow::Cow,
    collections::BTreeMap,
    env,
    ffi::OsString,
    io::{self, Write},
    path::PathBuf,
    process::Command,
};

use crate::{Args, Result};

// Refs:
// * https://github.com/rust-lang/cargo/blob/0.44.0/src/cargo/ops/cargo_output_metadata.rs#L56-L63
// * https://github.com/rust-lang/cargo/blob/0.44.0/src/cargo/core/package.rs#L57-L80
// * https://github.com/oli-obk/cargo_metadata

// As cargo_metadata does not preserve the order of feature flags, use our own structs.

#[derive(Deserialize)]
pub(crate) struct Metadata {
    /// A list of all crates referenced by this crate (and the crate itself)
    pub(crate) packages: Vec<Package>,
    /// Workspace root
    pub(crate) workspace_root: PathBuf,
}

#[derive(Deserialize)]
pub(crate) struct Package {
    /// Name as given in the `Cargo.toml`
    pub(crate) name: String,
    /// List of dependencies of this particular package
    pub(crate) dependencies: Vec<Dependency>,
    /// Features provided by the crate, mapped to the features required by that feature.
    pub(crate) features: BTreeMap<String, Vec<String>>,
    /// Path containing the `Cargo.toml`
    pub(crate) manifest_path: PathBuf,
}

#[derive(Deserialize)]
/// A dependency of the main crate
pub(crate) struct Dependency {
    /// Name as given in the `Cargo.toml`
    pub(crate) name: String,
    /// Whether this dependency is required or optional
    pub(crate) optional: bool,
    // TODO: support this
    // /// The target this dependency is specific to.
    // pub target: Option<String>,
    /// If the dependency is renamed, this is the new name for the dependency
    /// as a string.  None if it is not renamed.
    pub(crate) rename: Option<String>,
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

impl Dependency {
    pub(crate) fn as_feature(&self) -> Option<&String> {
        if self.optional { Some(self.rename.as_ref().unwrap_or(&self.name)) } else { None }
    }
}
