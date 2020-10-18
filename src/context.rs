use std::{
    collections::HashMap,
    env,
    ffi::{OsStr, OsString},
    ops::Deref,
    path::Path,
};

use crate::{
    cli::{self, Args, Coloring, RawArgs},
    manifest::{find_root_manifest_for_wd, Manifest},
    metadata::{Metadata, Package, PackageId},
    Result,
};

pub(crate) struct Context<'a> {
    args: Args<'a>,
    metadata: Metadata,
    current_manifest: Manifest,
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

        let current_manifest = match args.manifest_path {
            Some(path) => Manifest::new(Path::new(path))?,
            None => Manifest::new(find_root_manifest_for_wd(&env::current_dir()?)?)?,
        };

        let mut manifests = HashMap::with_capacity(metadata.workspace_members.len());
        for id in &metadata.workspace_members {
            let manifest_path = &metadata.packages[id].manifest_path;
            let manifest = Manifest::new(manifest_path)?;
            manifests.insert(id.clone(), manifest);
        }

        Ok(Self { args, metadata, current_manifest, manifests, cargo })
    }

    // Accessor methods.
    pub(crate) fn packages(&self, id: &PackageId) -> &Package {
        &self.metadata.packages[id]
    }
    pub(crate) fn workspace_members(&self) -> impl Iterator<Item = &PackageId> {
        self.metadata.workspace_members.iter()
    }
    // pub(crate) fn nodes(&self, id: &PackageId) -> &Node {
    //     &self.metadata.nodes[id]
    // }
    pub(crate) fn workspace_root(&self) -> &Path {
        &self.metadata.workspace_root
    }
    pub(crate) fn current_manifest(&self) -> &Manifest {
        &self.current_manifest
    }
    pub(crate) fn manifests(&self, id: &PackageId) -> &Manifest {
        &self.manifests[id]
    }
    pub(crate) fn cargo(&self) -> &OsStr {
        &self.cargo
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
