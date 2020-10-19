use std::{collections::HashMap, env, ffi::OsString, ops::Deref, path::Path};

use crate::{
    cli::{self, Args, RawArgs},
    manifest::Manifest,
    metadata::{Metadata, Node, Package, PackageId},
    version, ProcessBuilder, Result,
};

pub(crate) struct Context<'a> {
    args: Args<'a>,
    metadata: Metadata,
    manifests: HashMap<PackageId, Manifest>,
    cargo: OsString,
    cargo_version: u32,
}

impl<'a> Context<'a> {
    pub(crate) fn new(args: &'a RawArgs) -> Result<Self> {
        let cargo = cargo_binary();

        // If failed to determine cargo version, assign 0 to skip all version-dependent decisions.
        let cargo_version = match version::from_path(&cargo) {
            Ok(version) => version.minor,
            Err(e) => {
                warn!("{}", e);
                0
            }
        };

        let args = cli::parse_args(args, &cargo, cargo_version)?;
        assert!(
            args.subcommand.is_some() || args.remove_dev_deps,
            "no subcommand or valid flag specified"
        );
        let metadata = Metadata::new(&args, &cargo, cargo_version)?;

        // Only a few options require information from cargo manifest.
        // If manifest information is not required, do not read and parse them.
        let manifests = if args.require_manifest_info(cargo_version) {
            let mut manifests = HashMap::with_capacity(metadata.workspace_members.len());
            for id in &metadata.workspace_members {
                let manifest_path = &metadata.packages[id].manifest_path;
                let manifest = Manifest::new(manifest_path)?;
                manifests.insert(id.clone(), manifest);
            }
            manifests
        } else {
            HashMap::new()
        };

        Ok(Self { args, metadata, manifests, cargo, cargo_version })
    }

    // Accessor methods.
    pub(crate) fn packages(&self, id: &PackageId) -> &Package {
        &self.metadata.packages[id]
    }
    pub(crate) fn workspace_members(&self) -> impl Iterator<Item = &PackageId> {
        self.metadata.workspace_members.iter()
    }
    pub(crate) fn nodes(&self, id: &PackageId) -> &Node {
        &self.metadata.resolve.nodes[id]
    }
    pub(crate) fn current_package(&self) -> Option<&PackageId> {
        self.metadata.resolve.root.as_ref()
    }
    pub(crate) fn workspace_root(&self) -> &Path {
        &self.metadata.workspace_root
    }
    pub(crate) fn manifests(&self, id: &PackageId) -> &Manifest {
        debug_assert!(self.require_manifest_info(self.cargo_version));
        &self.manifests[id]
    }
    pub(crate) fn is_private(&self, id: &PackageId) -> bool {
        if self.cargo_version >= 39 {
            !self.packages(id).publish
        } else {
            !self.manifests(id).publish
        }
    }

    pub(crate) fn process(&self) -> ProcessBuilder<'_> {
        let mut command = ProcessBuilder::new(&self.cargo);
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

fn cargo_binary() -> OsString {
    env::var_os("CARGO_HACK_CARGO_SRC")
        .unwrap_or_else(|| env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}
