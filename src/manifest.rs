use std::path::Path;

use anyhow::{format_err, Context as _, Result};
use toml::{value::Table, Value};

use crate::fs;

type ParseResult<T> = Result<T, &'static str>;

// Cargo manifest
// https://doc.rust-lang.org/nightly/cargo/reference/manifest.html
pub(crate) struct Manifest {
    pub(crate) raw: String,
    pub(crate) package: Package,
}

impl Manifest {
    pub(crate) fn new(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)?;
        let toml = toml::from_str(&raw)
            .with_context(|| format!("failed to parse manifest `{}` as toml", path.display()))?;
        let package = Package::from_table(&toml).map_err(|s| {
            format_err!("failed to parse `{}` field from manifest `{}`", s, path.display())
        })?;
        Ok(Self { raw, package })
    }

    pub(crate) fn remove_dev_deps(&self) -> String {
        super::remove_dev_deps::remove_dev_deps(&self.raw)
    }
}

pub(crate) struct Package {
    // `metadata.package.publish` requires Rust 1.39
    pub(crate) publish: bool,
    // `metadata.package.rust_version` requires Rust 1.58
    pub(crate) rust_version: Option<String>,
}

impl Package {
    fn from_table(table: &Table) -> ParseResult<Self> {
        let package = table.get("package").and_then(Value::as_table).ok_or("package")?;

        Ok(Self {
            // Publishing is unrestricted if `true` or the field is not
            // specified, and forbidden if `false` or the array is empty.
            publish: match package.get("publish") {
                None => true,
                Some(Value::Boolean(b)) => *b,
                Some(Value::Array(a)) => !a.is_empty(),
                Some(_) => return Err("publish"),
            },
            rust_version: match package.get("rust-version").map(Value::as_str) {
                None => None,
                Some(Some(v)) => Some(v.to_owned()),
                Some(None) => return Err("rust-version"),
            },
        })
    }
}
