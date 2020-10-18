use anyhow::{format_err, Context as _};
use std::{fs, path::Path};
use toml::{value::Table, Value};

use crate::Result;

type ParseResult<T> = Result<T, &'static str>;

pub(crate) struct Manifest {
    pub(crate) raw: String,
    // parsed manifest
    pub(crate) package: Package,
}

impl Manifest {
    pub(crate) fn new(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read manifest from {}", path.display()))?;
        let toml = toml::from_str(&raw)
            .with_context(|| format!("failed to parse manifest as toml: {}", path.display()))?;
        let package = Package::from_table(toml).map_err(|s| {
            format_err!("failed to parse `{}` field from manifest: {}", s, path.display())
        })?;
        Ok(Self { raw, package })
    }

    // `metadata.package.publish` requires Rust 1.39
    pub(crate) fn is_private(&self) -> bool {
        self.package.publish == false
    }

    pub(crate) fn remove_dev_deps(&self) -> String {
        super::remove_dev_deps::remove_dev_deps(&self.raw)
    }
}

// Refs:
// * https://github.com/rust-lang/cargo/blob/0.47.0/src/cargo/util/toml/mod.rs
// * https://gitlab.com/crates.rs/cargo_toml

pub(crate) struct Package {
    pub(crate) name: String,
    pub(crate) publish: Publish,
}

impl Package {
    fn from_table(mut table: Table) -> ParseResult<Self> {
        let package = table.get_mut("package").and_then(Value::as_table_mut).ok_or("package")?;
        let name = package.remove("name").and_then(into_string).ok_or("name")?;
        let publish = Publish::from_value(package.get("publish")).ok_or("publish")?;

        Ok(Self { name, publish })
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
