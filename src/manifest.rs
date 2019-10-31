use std::{
    borrow::Cow,
    fs, ops,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use toml_edit::{Array, Document, Item as Value, Table};

use crate::Options;

const MANIFEST_FILE: &str = "Cargo.toml";

#[derive(Debug)]
pub(crate) struct Manifest {
    pub(crate) path: PathBuf,
    pub(crate) raw: String,
    pub(crate) toml: Document,
    pub(crate) features: Vec<String>,
}

impl Manifest {
    pub(crate) fn new(path: &Path) -> Result<Self> {
        let path = path
            .canonicalize()
            .with_context(|| format!("failed to canonicalize manifest path: {}", path.display()))?;
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read manifest from {}", path.display()))?;
        let toml: Document = raw.parse()?;
        let features = toml
            .as_table()
            .get("features")
            .and_then(Value::as_table)
            .into_iter()
            .flat_map(|f| f.iter().map(|(k, _)| k.to_string()))
            .collect::<Vec<_>>();

        let manifest = Self { path, raw, toml, features };
        if manifest.package().is_none() && manifest.members().is_none() {
            // TODO: improve error message
            bail!("expected 'package' or 'workspace'");
        }

        Ok(manifest)
    }

    pub(crate) fn with_manifest_path(path: &str) -> Result<Self> {
        if !path.ends_with(MANIFEST_FILE) {
            bail!("the manifest-path must be a path to a Cargo.toml file");
        }

        let path = Path::new(path);
        if !path.exists() {
            bail!("manifest path `{}` does not exist", path.display());
        }

        Self::new(path)
    }

    pub(crate) fn dir(&self) -> &Path {
        self.path.parent().unwrap()
    }

    pub(crate) fn package(&self) -> Option<&Table> {
        self.toml.as_table().get("package")?.as_table()
    }

    pub(crate) fn package_name(&self) -> &str {
        assert!(!self.is_virtual());
        (|| self.package()?.get("name")?.as_str())().unwrap()
    }

    pub(crate) fn package_name_verbose(&self, args: &Options) -> Cow<'_, str> {
        assert!(!self.is_virtual());
        if args.verbose {
            Cow::Owned(format!("{} ({})", self.package_name(), self.dir().display()))
        } else {
            Cow::Borrowed(self.package_name())
        }
    }

    pub(crate) fn is_virtual(&self) -> bool {
        self.package().is_none()
    }

    pub(crate) fn is_private(&self) -> bool {
        (|| self.package()?.get("publish")?.as_bool())().map_or(false, ops::Not::not)
    }

    pub(crate) fn workspace(&self) -> Option<&Table> {
        self.toml.as_table().get("workspace")?.as_table()
    }

    pub(crate) fn members(&self) -> Option<&Array> {
        (|| self.workspace()?.get("members")?.as_array())()
    }
}

pub(crate) fn remove_key_and_target_key(table: &mut Table, key: &str) {
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

// Based on https://github.com/rust-lang/cargo/blob/dc83ead224d8622f748f507574e1448a28d8dcc7/src/cargo/util/important_paths.rs

/// Finds the root `Cargo.toml`.
pub(crate) fn find_root_manifest_for_wd(cwd: &Path) -> Result<PathBuf> {
    for current in cwd.ancestors() {
        let manifest = current.join(MANIFEST_FILE);
        if manifest.exists() {
            return Ok(manifest);
        }
    }

    bail!("could not find `Cargo.toml` in `{}` or any parent directory", cwd.display())
}

/// Returns the path to the `MANIFEST_FILE` in `pwd`, if it exists.
pub(crate) fn find_project_manifest_exact(pwd: &Path) -> Result<PathBuf> {
    let manifest = pwd.join(MANIFEST_FILE);

    if manifest.exists() {
        Ok(manifest)
    } else {
        bail!("Could not find `{}` in `{}`", MANIFEST_FILE, pwd.display())
    }
}
