use anyhow::{format_err, Context as _};
use std::{fs, path::Path};
use toml::{value::Table, Value};

use crate::Result;

type ParseResult<T> = Result<T, &'static str>;

// Refs:
// * https://github.com/rust-lang/cargo/blob/0.47.0/src/cargo/util/toml/mod.rs
// * https://gitlab.com/crates.rs/cargo_toml

pub(crate) struct Manifest {
    pub(crate) raw: String,
    // `metadata.package.publish` requires Rust 1.39
    pub(crate) publish: bool,
}

impl Manifest {
    pub(crate) fn new(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read manifest from `{}`", path.display()))?;
        let toml = toml::from_str(&raw)
            .with_context(|| format!("failed to parse manifest `{}` as toml", path.display()))?;
        let package = Package::from_table(&toml).map_err(|s| {
            format_err!("failed to parse `{}` field from manifest `{}`", s, path.display())
        })?;
        Ok(Self { raw, publish: package.publish })
    }

    pub(crate) fn remove_dev_deps(&self) -> String {
        super::remove_dev_deps::remove_dev_deps(&self.raw)
    }
}

struct Package {
    publish: bool,
}

impl Package {
    fn from_table(table: &Table) -> ParseResult<Self> {
        let package = table.get("package").and_then(Value::as_table).ok_or("package")?;
        let _name = package.get("name").and_then(Value::as_str).ok_or("name")?;

        Ok(Self {
            // Publishing is unrestricted if `true`, and forbidden if `false` or the `Array` is empty.
            publish: match package.get("publish") {
                None => true,
                Some(Value::Boolean(b)) => *b,
                Some(Value::Array(a)) => !a.is_empty(),
                Some(_) => return Err("publish"),
            },
        })
    }
}
