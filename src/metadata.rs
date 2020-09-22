use anyhow::{format_err, Context};
use serde_json::Value;
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

pub(crate) struct Metadata {
    /// A list of all crates referenced by this crate (and the crate itself)
    pub(crate) packages: Vec<Package>,
    /// Workspace root
    pub(crate) workspace_root: PathBuf,
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

        let value = serde_json::from_slice(&output.stdout).context("failed to parse metadata")?;
        Self::from_value(&value).ok_or_else(|| format_err!("failed to parse metadata"))
    }

    fn from_value(value: &Value) -> Option<Self> {
        let map = value.as_object()?;
        let packages = map
            .get("packages")?
            .as_array()?
            .iter()
            .map(Package::from_value)
            .collect::<Option<Vec<_>>>()?;
        let workspace_root = map.get("workspace_root")?.as_str()?.to_string().into();

        Some(Self { packages, workspace_root })
    }
}

pub(crate) struct Package {
    /// Name as given in the `Cargo.toml`
    pub(crate) name: String,
    /// List of dependencies of this particular package
    pub(crate) dependencies: Vec<Dependency>,
    /// Features provided by the crate, mapped to the features required by that feature.
    pub(crate) features: BTreeMap<String, Vec<Value>>,
    /// Path containing the `Cargo.toml`
    pub(crate) manifest_path: PathBuf,
}

impl Package {
    fn from_value(value: &Value) -> Option<Self> {
        let map = value.as_object()?;
        let name = map.get("name")?.as_str()?.to_string();
        let dependencies = map
            .get("dependencies")?
            .as_array()?
            .iter()
            .map(Dependency::from_value)
            .collect::<Option<Vec<_>>>()?;
        let features = map
            .get("features")?
            .as_object()?
            .iter()
            .map(|(k, v)| v.as_array().map(|a| (k.to_string(), a.to_vec())))
            .collect::<Option<BTreeMap<_, _>>>()?;
        let manifest_path = map.get("manifest_path")?.as_str()?.to_string().into();

        Some(Self { name, dependencies, features, manifest_path })
    }

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

impl Dependency {
    fn from_value(value: &Value) -> Option<Self> {
        let map = value.as_object()?;
        let name = map.get("name")?.as_str()?.to_string();
        let optional = map.get("optional")?.as_bool()?;
        let rename = map.get("rename")?;
        let rename = if rename.is_null() { None } else { Some(rename.as_str()?.to_string()) };
        Some(Self { name, optional, rename })
    }

    pub(crate) fn as_feature(&self) -> Option<&String> {
        if self.optional { Some(self.rename.as_ref().unwrap_or(&self.name)) } else { None }
    }
}
