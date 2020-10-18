use std::{collections::HashMap, env, ffi::OsString, ops::Deref, path::Path};

use crate::{
    cli::{self, Args, Coloring, RawArgs},
    manifest::Manifest,
    metadata::{Metadata, Package, PackageId},
    ProcessBuilder, Result,
};

pub(crate) struct Context<'a> {
    args: Args<'a>,
    metadata: Metadata,
    manifests: HashMap<PackageId, Manifest>,
    cargo: OsString,
}

impl<'a> Context<'a> {
    pub(crate) fn new(args: &'a RawArgs, coloring: &mut Option<Coloring>) -> Result<Self> {
        let cargo = cargo_binary();
        let args = cli::perse_args(args, coloring, &cargo)?;
        assert!(
            args.subcommand.is_some() || args.remove_dev_deps,
            "no subcommand or valid flag specified"
        );
        let metadata = Metadata::new(&args, &cargo)?;

        let mut manifests = HashMap::with_capacity(metadata.workspace_members.len());
        for id in &metadata.workspace_members {
            let manifest_path = &metadata.packages[id].manifest_path;
            let manifest = Manifest::new(manifest_path)?;
            manifests.insert(id.clone(), manifest);
        }

        Ok(Self { args, metadata, manifests, cargo })
    }

    // Accessor methods.
    pub(crate) fn packages(&self, id: &PackageId) -> &Package {
        &self.metadata.packages[id]
    }
    pub(crate) fn workspace_members(&self) -> impl Iterator<Item = &PackageId> {
        self.metadata.workspace_members.iter()
    }
    // pub(crate) fn nodes(&self, id: &PackageId) -> &Node {
    //     &self.metadata.resolve.nodes[id]
    // }
    pub(crate) fn current_manifest(&self) -> Option<&PackageId> {
        self.metadata.resolve.root.as_ref()
    }
    pub(crate) fn workspace_root(&self) -> &Path {
        &self.metadata.workspace_root
    }
    pub(crate) fn manifests(&self, id: &PackageId) -> &Manifest {
        &self.manifests[id]
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
