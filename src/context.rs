// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    borrow::Cow,
    collections::HashMap,
    env,
    ffi::OsString,
    ops,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context as _, Result};

use crate::{
    cargo,
    cli::Args,
    features::Features,
    manifest::Manifest,
    metadata::{Metadata, Package, PackageId},
    restore, term, ProcessBuilder,
};

pub(crate) struct Context {
    args: Args,
    pub(crate) metadata: Metadata,
    manifests: HashMap<PackageId, Manifest>,
    pkg_features: HashMap<PackageId, Features>,
    cargo: PathBuf,
    pub(crate) cargo_version: u32,
    pub(crate) restore: restore::Manager,
    pub(crate) current_dir: PathBuf,
    pub(crate) current_package: Option<PackageId>,
}

impl Context {
    pub(crate) fn new(args: Args, cargo: OsString) -> Result<Self> {
        assert!(
            args.subcommand.is_some() || args.remove_dev_deps,
            "no subcommand or valid flag specified"
        );

        // If failed to determine cargo version, assign 0 to skip all version-dependent decisions.
        let cargo_version = cargo::version(cmd!(&cargo))
            .map_err(|e| warn!("unable to determine cargo version: {e:#}"))
            .map(|v| v.minor)
            .unwrap_or(0);

        // if `--remove-dev-deps` flag is off, restore manifest file.
        let mut restore = restore::Manager::new(!args.remove_dev_deps);
        let metadata = Metadata::new(
            args.manifest_path.as_deref(),
            &cargo,
            cargo_version,
            &args,
            &mut restore,
        )?;
        if metadata.cargo_version < 41 && args.include_deps_features {
            bail!("--include-deps-features requires Cargo 1.41 or later");
        }

        let mut manifests = HashMap::with_capacity(metadata.workspace_members.len());
        let mut pkg_features = HashMap::with_capacity(metadata.workspace_members.len());

        for id in &metadata.workspace_members {
            let manifest_path = &metadata.packages[id].manifest_path;
            let manifest = Manifest::new(manifest_path, metadata.cargo_version)?;
            let features = Features::new(&metadata, &manifest, id, args.include_deps_features);
            manifests.insert(id.clone(), manifest);
            pkg_features.insert(id.clone(), features);
        }

        let mut cmd = cmd!(&cargo, "locate-project");
        if let Some(manifest_path) = &args.manifest_path {
            cmd.arg("--manifest-path");
            cmd.arg(manifest_path);
        }
        // Use json format because `--message-format plain` option of
        // `cargo locate-project` has been added in Rust 1.48.
        let locate_project: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&cmd.read()?)
                .with_context(|| format!("failed to parse output from {cmd}"))?;
        let locate_project = Path::new(locate_project["root"].as_str().unwrap());
        let mut current_package = None;
        for id in &metadata.workspace_members {
            let manifest_path = &metadata.packages[id].manifest_path;
            // no need to use same_file as cargo-metadata and cargo-locate-project
            // as they return absolute paths resolved in the same way.
            if locate_project == manifest_path {
                current_package = Some(id.clone());
                break;
            }
        }

        let this = Self {
            args,
            metadata,
            manifests,
            pkg_features,
            cargo: cargo.into(),
            cargo_version,
            restore,
            current_dir: env::current_dir()?,
            current_package,
        };

        // TODO: Ideally, we should do this, but for now, we allow it as cargo-hack
        // may mistakenly interpret the specified valid feature flag as unknown.
        // if this.ignore_unknown_features && !this.workspace && !this.current_manifest().is_virtual() {
        //     bail!(
        //         "--ignore-unknown-features can only be used in the root of a virtual workspace or together with --workspace"
        //     )
        // }

        Ok(this)
    }

    // Accessor methods.

    pub(crate) fn packages(&self, id: &PackageId) -> &Package {
        &self.metadata.packages[id]
    }

    pub(crate) fn workspace_members(&self) -> impl ExactSizeIterator<Item = &PackageId> {
        self.metadata.workspace_members.iter()
    }

    pub(crate) fn current_package(&self) -> Option<&PackageId> {
        self.current_package.as_ref()
    }

    pub(crate) fn workspace_root(&self) -> &Path {
        &self.metadata.workspace_root
    }

    pub(crate) fn manifests(&self, id: &PackageId) -> &Manifest {
        &self.manifests[id]
    }

    pub(crate) fn pkg_features(&self, id: &PackageId) -> &Features {
        &self.pkg_features[id]
    }

    pub(crate) fn is_private(&self, id: &PackageId) -> bool {
        if self.metadata.cargo_version >= 39 {
            !self.packages(id).publish
        } else {
            !self.manifests(id).package.publish.unwrap()
        }
    }

    pub(crate) fn rust_version(&self, id: &PackageId) -> Option<&str> {
        if self.metadata.cargo_version >= 58 {
            self.packages(id).rust_version.as_deref()
        } else {
            self.manifests(id).package.rust_version.as_ref().unwrap().as_deref()
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
