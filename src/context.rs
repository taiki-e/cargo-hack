use std::{
    borrow::Cow,
    collections::HashMap,
    env,
    ffi::OsString,
    ops,
    path::{Path, PathBuf},
};

use anyhow::{bail, Result};

use crate::{
    cli::Args,
    features::Features,
    manifest::Manifest,
    metadata::{Metadata, Package, PackageId},
    restore, rustup, term, ProcessBuilder,
};

pub(crate) struct Context {
    args: Args,
    metadata: Metadata,
    manifests: HashMap<PackageId, Manifest>,
    pkg_features: HashMap<PackageId, Features>,
    cargo: PathBuf,
    pub(crate) restore: restore::Manager,
    pub(crate) current_dir: PathBuf,
    pub(crate) version_range: Option<Vec<String>>,
}

impl Context {
    pub(crate) fn new() -> Result<Self> {
        let cargo = env::var_os("CARGO_HACK_CARGO_SRC")
            .unwrap_or_else(|| env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")));
        let args = Args::parse(&cargo)?;
        assert!(
            args.subcommand.is_some() || args.remove_dev_deps,
            "no subcommand or valid flag specified"
        );

        let mut restore = restore::Manager::new(true);
        let metadata = Metadata::new(&args, &cargo, &restore)?;
        // if `--remove-dev-deps` flag is off, restore manifest file.
        restore.needs_restore = args.no_dev_deps && !args.remove_dev_deps;
        if metadata.cargo_version < 41 && args.include_deps_features {
            bail!("--include-deps-features requires Cargo 1.41 or later");
        }

        let mut pkg_features = HashMap::with_capacity(metadata.workspace_members.len());
        for id in &metadata.workspace_members {
            let features = Features::new(&metadata, id);
            pkg_features.insert(id.clone(), features);
        }

        let mut this = Self {
            args,
            metadata,
            manifests: HashMap::new(),
            pkg_features,
            cargo: cargo.into(),
            restore,
            current_dir: env::current_dir()?,
            version_range: None,
        };

        // Only a few options require information from cargo manifest.
        // If manifest information is not required, do not read and parse them.
        if this.require_manifest_info() {
            this.manifests.reserve(this.metadata.workspace_members.len());
            for id in &this.metadata.workspace_members {
                let manifest_path = &this.metadata.packages[id].manifest_path;
                let manifest = Manifest::new(manifest_path)?;
                this.manifests.insert(id.clone(), manifest);
            }
        }

        this.version_range = this
            .args
            .version_range
            .as_ref()
            .map(|range| rustup::version_range(range, this.args.version_step.as_deref(), &this))
            .transpose()?;

        Ok(this)
    }

    // Accessor methods.

    pub(crate) fn packages(&self, id: &PackageId) -> &Package {
        &self.metadata.packages[id]
    }

    pub(crate) fn workspace_members(&self) -> impl Iterator<Item = &PackageId> {
        self.metadata.workspace_members.iter()
    }

    pub(crate) fn current_package(&self) -> Option<&PackageId> {
        self.metadata.resolve.root.as_ref()
    }

    pub(crate) fn workspace_root(&self) -> &Path {
        &self.metadata.workspace_root
    }

    pub(crate) fn manifests(&self, id: &PackageId) -> &Manifest {
        debug_assert!(self.require_manifest_info());
        &self.manifests[id]
    }

    pub(crate) fn pkg_features(&self, id: &PackageId) -> &Features {
        &self.pkg_features[id]
    }

    pub(crate) fn is_private(&self, id: &PackageId) -> bool {
        if self.metadata.cargo_version >= 39 {
            !self.packages(id).publish
        } else {
            !self.manifests(id).package.publish
        }
    }

    pub(crate) fn rust_version(&self, id: &PackageId) -> Option<&str> {
        if self.metadata.cargo_version >= 58 {
            self.packages(id).rust_version.as_deref()
        } else {
            self.manifests(id).package.rust_version.as_deref()
        }
    }

    pub(crate) fn name_verbose(&self, id: &PackageId) -> Cow<'_, str> {
        let package = self.packages(id);
        if term::verbose() {
            Cow::Owned(format!(
                "{} ({})",
                package.name,
                package.manifest_path.parent().unwrap().display()
            ))
        } else {
            Cow::Borrowed(&package.name)
        }
    }

    /// Return `true` if options that require information from cargo manifest is specified.
    pub(crate) fn require_manifest_info(&self) -> bool {
        (self.metadata.cargo_version < 39 && self.ignore_private)
            || (self.metadata.cargo_version < 58 && self.args.version_range.is_some())
            || self.no_dev_deps
            || self.remove_dev_deps
    }

    pub(crate) fn cargo(&self) -> ProcessBuilder<'_> {
        cmd!(&self.cargo)
    }
}

impl ops::Deref for Context {
    type Target = Args;

    fn deref(&self) -> &Self::Target {
        &self.args
    }
}
