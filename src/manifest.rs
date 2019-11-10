use std::{
    fs, ops,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use toml_edit::{Document, Table};

#[derive(Clone, Debug)]
pub(crate) struct Manifest {
    pub(crate) path: PathBuf,
    pub(crate) raw: String,
    pub(crate) doc: Document,
}

impl Manifest {
    pub(crate) fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read manifest from {}", path.display()))?;
        let doc: Document = raw
            .parse()
            .with_context(|| format!("failed to parse manifest file: {}", path.display()))?;
        Ok(Self { path, raw, doc })
    }

    pub(crate) fn package(&self) -> Option<&Table> {
        self.doc.as_table().get("package")?.as_table()
    }

    pub(crate) fn package_name(&self) -> &str {
        assert!(!self.is_virtual());
        (|| self.package()?.get("name")?.as_str())().unwrap()
    }

    pub(crate) fn is_virtual(&self) -> bool {
        self.package().is_none()
    }

    // `metadata.package.publish` requires Rust 1.39
    pub(crate) fn is_private(&self) -> bool {
        (|| self.package()?.get("publish")?.as_bool())().map_or(false, ops::Not::not)
    }

    pub(crate) fn remove_dev_deps(&mut self) -> String {
        remove_key_and_target_key(self.doc.as_table_mut(), "dev-dependencies");
        self.doc.to_string_in_original_order()
    }
}

fn remove_key_and_target_key(table: &mut Table, key: &str) {
    table.remove(key);
    if let Some(table) = table.entry("target").as_table_mut() {
        // `toml_edit::Table` does not have `.iter_mut()`, so collect keys.
        for k in table.iter().map(|(key, _)| key.to_string()).collect::<Vec<_>>() {
            if let Some(table) = table.entry(&k).as_table_mut() {
                table.remove(key);
            }
        }
    }
}

// Based on https://github.com/rust-lang/cargo/blob/0.39.0/src/cargo/util/important_paths.rs

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
