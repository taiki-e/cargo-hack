use anyhow::{bail, Context};
use std::{
    fs,
    path::{Path, PathBuf},
};
use toml::{value::Table, Value};

use crate::Result;

pub(crate) struct Manifest {
    pub(crate) path: PathBuf,
    pub(crate) raw: String,

    // parsed manifest
    // if `None`, workspace is virtual
    pub(crate) package: Option<Package>,
}

impl Manifest {
    pub(crate) fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read manifest from {}", path.display()))?;
        let toml = toml::from_str(&raw)
            .with_context(|| format!("failed to parse manifest file: {}", path.display()))?;
        let package = Package::from_table(toml);
        Ok(Self { path, raw, package })
    }

    pub(crate) fn package_name(&self) -> &str {
        assert!(!self.is_virtual());
        &self.package.as_ref().unwrap().name
    }

    pub(crate) fn is_virtual(&self) -> bool {
        self.package.is_none()
    }

    // `metadata.package.publish` requires Rust 1.39
    pub(crate) fn is_private(&self) -> bool {
        assert!(!self.is_virtual());
        self.package.as_ref().unwrap().publish == false
    }

    pub(crate) fn remove_dev_deps(&self) -> String {
        super::remove_dev_deps::remove_dev_deps(&self.raw)
    }
}

// Based on https://github.com/rust-lang/cargo/blob/0.44.0/src/cargo/util/important_paths.rs
/// Finds the root `Cargo.toml`.
pub(crate) fn find_root_manifest_for_wd(cwd: &Path) -> Result<PathBuf> {
    for current in cwd.ancestors() {
        let manifest = current.join("Cargo.toml");
        if manifest.exists() {
            return Ok(manifest);
        }
    }

    bail!("could not find `Cargo.toml` in `{}` or any parent directory", cwd.display())
}

// Refs:
// * https://github.com/rust-lang/cargo/blob/0.44.0/src/cargo/util/toml/mod.rs
// * https://gitlab.com/crates.rs/cargo_toml

pub(crate) struct Package {
    pub(crate) name: String,
    pub(crate) publish: Publish,
}

impl Package {
    fn from_table(mut table: Table) -> Option<Self> {
        let package = table.get_mut("package")?.as_table_mut()?;
        let name = into_string(package.remove("name")?)?;
        let publish = Publish::from_value(package.get("publish"))?;

        Some(Self { name, publish })
    }
}

pub(crate) enum Publish {
    Flag(bool),
    Registry { is_empty: bool },
}

impl Publish {
    fn from_value(value: Option<&Value>) -> Option<Self> {
        Some(match value {
            None => Self::default(),
            Some(Value::Array(a)) => Publish::Registry { is_empty: a.is_empty() },
            Some(Value::Boolean(b)) => Publish::Flag(*b),
            Some(_) => return None,
        })
    }
}

impl Default for Publish {
    fn default() -> Self {
        Publish::Flag(true)
    }
}

impl PartialEq<Publish> for bool {
    fn eq(&self, p: &Publish) -> bool {
        match *p {
            Publish::Flag(flag) => flag == *self,
            Publish::Registry { is_empty } => is_empty != *self,
        }
    }
}

impl PartialEq<bool> for Publish {
    fn eq(&self, b: &bool) -> bool {
        b.eq(self)
    }
}

fn into_string(value: Value) -> Option<String> {
    if let Value::String(string) = value { Some(string) } else { None }
}
