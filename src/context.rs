use std::{
    borrow::Cow,
    collections::HashMap,
    env,
    ops::Deref,
    path::{Path, PathBuf},
};

use crate::{
    cli::{self, Args, RawArgs},
    features::Features,
    manifest::Manifest,
    metadata::{Metadata, Package, PackageId},
    Cargo, ProcessBuilder, Result,
};

pub(crate) struct Context<'a> {
    args: Args<'a>,
    metadata: Metadata,
    manifests: HashMap<PackageId, Manifest>,
    pkg_features: HashMap<PackageId, Features>,
    cargo: Cargo,
    pub(crate) current_dir: PathBuf,
}

impl<'a> Context<'a> {
    pub(crate) fn new(args: &'a RawArgs) -> Result<Self> {
        let cargo = Cargo::new();
        let current_dir = env::current_dir()?;

        let args = cli::parse_args(args, &cargo)?;
        assert!(
            args.subcommand.is_some() || args.remove_dev_deps,
            "no subcommand or valid flag specified"
        );

        let metadata = Metadata::new(&args, &cargo)?;

        let mut pkg_features = HashMap::with_capacity(metadata.workspace_members.len());
        for id in &metadata.workspace_members {
            let features = Features::new(&metadata, id);
            pkg_features.insert(id.clone(), features);
        }

        let mut this =
            Self { args, metadata, manifests: HashMap::new(), pkg_features, cargo, current_dir };

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
        if self.cargo.version >= 39 {
            !self.packages(id).publish
        } else {
            !self.manifests(id).publish
        }
    }

    pub(crate) fn name_verbose(&self, id: &PackageId) -> Cow<'_, str> {
        let package = self.packages(id);
        if self.verbose {
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
        (self.cargo.version < 39 && self.ignore_private) || self.no_dev_deps || self.remove_dev_deps
    }

    pub(crate) fn process(&self) -> ProcessBuilder<'_> {
        let mut cmd = self.cargo.process();
        if self.verbose {
            cmd.display_manifest_path();
        }
        cmd
    }
}

impl<'a> Deref for Context<'a> {
    type Target = Args<'a>;

    fn deref(&self) -> &Self::Target {
        &self.args
    }
}
