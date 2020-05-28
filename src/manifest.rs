use anyhow::{bail, Context};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::Result;

#[derive(Debug)]
pub(crate) struct Manifest {
    pub(crate) path: PathBuf,
    pub(crate) raw: String,

    // parsed manifest
    pub(crate) package: Option<de::Package>,
}

impl Manifest {
    pub(crate) fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read manifest from {}", path.display()))?;
        let toml: de::Manifest = toml::from_str(&raw)
            .with_context(|| format!("failed to parse manifest file: {}", path.display()))?;
        Ok(Self { path, raw, package: toml.package })
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

mod de {
    use serde_derive::Deserialize;

    // Refs:
    // * https://github.com/rust-lang/cargo/blob/0.44.0/src/cargo/util/toml/mod.rs
    // * https://gitlab.com/crates.rs/cargo_toml

    #[derive(Debug, Deserialize)]
    pub(crate) struct Manifest {
        pub(crate) package: Option<Package>,
    }

    #[derive(Debug, Deserialize)]
    pub(crate) struct Package {
        pub(crate) name: String,
        #[serde(default)]
        pub(crate) publish: Publish,
    }

    #[derive(Debug, Deserialize)]
    #[serde(untagged)]
    pub(crate) enum Publish {
        Flag(bool),
        Registry(Vec<String>),
    }

    impl Default for Publish {
        fn default() -> Self {
            Publish::Flag(true)
        }
    }

    impl PartialEq<Publish> for bool {
        fn eq(&self, p: &Publish) -> bool {
            match p {
                Publish::Flag(flag) => *flag == *self,
                Publish::Registry(reg) => reg.is_empty() != *self,
            }
        }
    }

    impl PartialEq<bool> for Publish {
        fn eq(&self, b: &bool) -> bool {
            b.eq(self)
        }
    }
}
