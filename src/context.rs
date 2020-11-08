use std::{collections::HashMap, ops::Deref, path::Path};

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
}

impl<'a> Context<'a> {
    pub(crate) fn new(args: &'a RawArgs) -> Result<Self> {
        let mut cargo = Cargo::new();

        let args = cli::parse_args(args, &mut cargo)?;
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

        let mut this = Self { args, metadata, manifests: HashMap::new(), pkg_features, cargo };

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
        if self.cargo.metadata() >= 39 {
            !self.packages(id).publish
        } else {
            !self.manifests(id).publish
        }
    }

    /// Return `true` if options that require information from cargo manifest is specified.
    pub(crate) fn require_manifest_info(&self) -> bool {
        (self.cargo.metadata() < 39 && self.ignore_private)
            || self.no_dev_deps
            || self.remove_dev_deps
    }

    pub(crate) fn process(&self) -> ProcessBuilder<'_> {
        let mut command = self.cargo.process();
        if self.verbose {
            command.display_manifest_path();
        }
        command
    }
}

impl<'a> Deref for Context<'a> {
    type Target = Args<'a>;

    fn deref(&self) -> &Self::Target {
        &self.args
    }
}
