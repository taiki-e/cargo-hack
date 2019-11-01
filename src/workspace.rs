use std::path::Path;

use anyhow::Result;
use indexmap::IndexMap;
use toml_edit::Item as Value;

use crate::manifest::{find_project_manifest_exact, Manifest};

pub(crate) struct Workspace<'a> {
    /// This manifest is a manifest to where the current cargo subcommand was
    /// invoked from.
    pub(crate) current_manifest: &'a Manifest,

    // Map of members in this workspace. Keys are their package names and values
    // are their manifests.
    pub(crate) members: IndexMap<String, Manifest>,
}

impl<'a> Workspace<'a> {
    pub(crate) fn new(current_manifest: &'a Manifest) -> Result<Self> {
        let mut members = IndexMap::new();
        // TODO: The current cargo-hack doesn't try to find the root manifest
        // after finding the current package's manifest.
        // https://github.com/taiki-e/cargo-hack/issues/11
        let root_dir = current_manifest.dir();
        let mut inserted = false;

        if let Some(workspace) = current_manifest.workspace() {
            for mut dir in workspace
                .get("members")
                .and_then(Value::as_array)
                .into_iter()
                .flat_map(|v| v.iter().filter_map(|v| v.as_str()))
                .map(Path::new)
            {
                if let Ok(new) = dir.strip_prefix(".") {
                    dir = new;
                }

                let path = find_project_manifest_exact(&root_dir.join(dir))?;
                let manifest = Manifest::new(&path)?;

                if current_manifest.path == manifest.path {
                    inserted = true;
                }
                members.insert(manifest.package_name().to_string(), manifest);
            }
        }

        if !current_manifest.is_virtual() && !inserted {
            members.insert(current_manifest.package_name().to_string(), current_manifest.clone());
        }

        Ok(Self { current_manifest, members })
    }
}
