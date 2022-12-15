// Refs:
// - https://doc.rust-lang.org/nightly/cargo/commands/cargo-metadata.html#output-format
// - https://github.com/rust-lang/cargo/blob/0.47.0/src/cargo/ops/cargo_output_metadata.rs#L56-L63
// - https://github.com/rust-lang/cargo/blob/0.47.0/src/cargo/core/package.rs#L57-L80
// - https://github.com/oli-obk/cargo_metadata

use std::{
    collections::{BTreeMap, HashMap},
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{format_err, Context as _, Result};
use serde_json::{Map, Value};

use crate::{cargo, cli::Args, fs, restore, term};

type Object = Map<String, Value>;
type ParseResult<T> = Result<T, &'static str>;

/// An opaque unique identifier for referring to the package.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct PackageId {
    /// The underlying string representation of id.
    /// The precise format is an implementation detail and is subject to change.
    repr: Arc<str>,
}

impl From<String> for PackageId {
    fn from(repr: String) -> Self {
        Self { repr: repr.into() }
    }
}

pub(crate) struct Metadata {
    pub(crate) cargo_version: u32,
    /// List of all packages in the workspace and all feature-enabled dependencies.
    pub(crate) packages: HashMap<PackageId, Package>,
    /// List of members of the workspace.
    pub(crate) workspace_members: Vec<PackageId>,
    /// The resolved dependency graph for the entire workspace.
    pub(crate) resolve: Resolve,
    /// The absolute path to the root of the workspace.
    pub(crate) workspace_root: PathBuf,
}

impl Metadata {
    pub(crate) fn new(
        args: &Args,
        cargo: &OsStr,
        mut cargo_version: u32,
        restore: &restore::Manager,
    ) -> Result<Self> {
        let stable_cargo_version = cargo::minor_version(cmd!("cargo", "+stable")).unwrap_or(0);

        let mut cmd;
        let json = if stable_cargo_version > cargo_version {
            cmd = cmd!(cargo, "metadata", "--format-version=1", "--no-deps");
            if let Some(manifest_path) = &args.manifest_path {
                cmd.arg("--manifest-path");
                cmd.arg(manifest_path);
            }
            let no_deps: Object = serde_json::from_str(&cmd.read()?)
                .with_context(|| format!("failed to parse output from {cmd}"))?;
            let lockfile =
                Path::new(no_deps["workspace_root"].as_str().unwrap()).join("Cargo.lock");
            if !lockfile.exists() {
                let mut cmd = cmd!(cargo, "generate-lockfile");
                if let Some(manifest_path) = &args.manifest_path {
                    cmd.arg("--manifest-path");
                    cmd.arg(manifest_path);
                }
                cmd.run_with_output()?;
            }
            let guard = term::verbose::scoped(false);
            let mut handle = restore.register_always(&fs::read_to_string(&lockfile)?, lockfile);
            // Try with stable cargo because if workspace member has
            // a dependency that requires newer cargo features, `cargo metadata`
            // with older cargo may fail.
            cmd = cmd!("cargo", "+stable", "metadata", "--format-version=1");
            if let Some(manifest_path) = &args.manifest_path {
                cmd.arg("--manifest-path");
                cmd.arg(manifest_path);
            }
            let json = cmd.read();
            handle.close()?;
            drop(guard);
            match json {
                Ok(json) => {
                    cargo_version = stable_cargo_version;
                    json
                }
                Err(_e) => {
                    // If failed, try again with the version of cargo we will actually use.
                    cmd = cmd!(cargo, "metadata", "--format-version=1");
                    if let Some(manifest_path) = &args.manifest_path {
                        cmd.arg("--manifest-path");
                        cmd.arg(manifest_path);
                    }
                    cmd.read()?
                }
            }
        } else {
            cmd = cmd!(cargo, "metadata", "--format-version=1");
            if let Some(manifest_path) = &args.manifest_path {
                cmd.arg("--manifest-path");
                cmd.arg(manifest_path);
            }
            cmd.read()?
        };

        let map = serde_json::from_str(&json)
            .with_context(|| format!("failed to parse output from {cmd}"))?;
        Self::from_obj(map, cargo_version)
            .map_err(|s| format_err!("failed to parse `{s}` field from metadata"))
    }

    fn from_obj(mut map: Object, cargo_version: u32) -> ParseResult<Self> {
        let workspace_members: Vec<_> = map
            .remove_array("workspace_members")?
            .into_iter()
            .map(|v| into_string(v).ok_or("workspace_members"))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            cargo_version,
            packages: map
                .remove_array("packages")?
                .into_iter()
                .map(|v| Package::from_value(v, cargo_version))
                .collect::<Result<_, _>>()?,
            workspace_members,
            resolve: Resolve::from_obj(map.remove_object("resolve")?, cargo_version)?,
            workspace_root: map.remove_string("workspace_root")?,
        })
    }
}

/// The resolved dependency graph for the entire workspace.
pub(crate) struct Resolve {
    /// Nodes in a dependency graph.
    pub(crate) nodes: HashMap<PackageId, Node>,
    /// The crate for which the metadata was read.
    /// This is `None` if the metadata was read in a virtual workspace.
    pub(crate) root: Option<PackageId>,
}

impl Resolve {
    fn from_obj(mut map: Object, cargo_version: u32) -> ParseResult<Self> {
        Ok(Self {
            nodes: map
                .remove_array("nodes")?
                .into_iter()
                .map(|v| Node::from_value(v, cargo_version))
                .collect::<Result<_, _>>()?,
            root: map.remove_nullable("root", into_string)?,
        })
    }
}

/// A node in a dependency graph.
pub(crate) struct Node {
    /// The dependencies of this package.
    ///
    /// This is always empty if running with a version of Cargo older than 1.30.
    pub(crate) deps: Vec<NodeDep>,
}

impl Node {
    fn from_value(mut value: Value, cargo_version: u32) -> ParseResult<(PackageId, Self)> {
        let map = value.as_object_mut().ok_or("nodes")?;

        let id = map.remove_string("id")?;
        Ok((id, Self {
            // This field was added in Rust 1.30.
            deps: if cargo_version >= 30 {
                map.remove_array("deps")?
                    .into_iter()
                    .map(|v| NodeDep::from_value(v, cargo_version))
                    .collect::<Result<_, _>>()?
            } else {
                Vec::new()
            },
        }))
    }
}

/// A dependency in a node.
pub(crate) struct NodeDep {
    /// The Package ID of the dependency.
    pub(crate) pkg: PackageId,
    /// The kinds of dependencies.
    ///
    /// This is always empty if running with a version of Cargo older than 1.41.
    pub(crate) dep_kinds: Vec<DepKindInfo>,
}

impl NodeDep {
    fn from_value(mut value: Value, cargo_version: u32) -> ParseResult<Self> {
        let map = value.as_object_mut().ok_or("deps")?;

        Ok(Self {
            pkg: map.remove_string("pkg")?,
            // This field was added in Rust 1.41.
            dep_kinds: if cargo_version >= 41 {
                map.remove_array("dep_kinds")?
                    .into_iter()
                    .map(DepKindInfo::from_value)
                    .collect::<Result<_, _>>()?
            } else {
                Vec::new()
            },
        })
    }
}

/// Information about a dependency kind.
pub(crate) struct DepKindInfo {
    /// The kind of dependency.
    pub(crate) kind: Option<String>,
    /// The target platform for the dependency.
    /// This is `None` if it is not a target dependency.
    pub(crate) target: Option<String>,
}

impl DepKindInfo {
    fn from_value(mut value: Value) -> ParseResult<Self> {
        let map = value.as_object_mut().ok_or("dep_kinds")?;

        Ok(Self {
            kind: map.remove_nullable("kind", into_string)?,
            target: map.remove_nullable("target", into_string)?,
        })
    }
}

pub(crate) struct Package {
    /// The name of the package.
    pub(crate) name: String,
    // /// The version of the package.
    // pub(crate) version: String,
    /// List of dependencies of this particular package.
    pub(crate) dependencies: Vec<Dependency>,
    /// Features provided by the crate, mapped to the features required by that feature.
    pub(crate) features: BTreeMap<String, Vec<String>>,
    /// Absolute path to this package's manifest.
    pub(crate) manifest_path: PathBuf,
    /// List of registries to which this package may be published.
    ///
    /// This is always `true` if running with a version of Cargo older than 1.39.
    pub(crate) publish: bool,
    /// The minimum supported Rust version of this package.
    ///
    /// This is always `None` if running with a version of Cargo older than 1.58.
    #[allow(dead_code)]
    pub(crate) rust_version: Option<String>,
}

impl Package {
    fn from_value(mut value: Value, cargo_version: u32) -> ParseResult<(PackageId, Self)> {
        let map = value.as_object_mut().ok_or("packages")?;

        let id = map.remove_string("id")?;
        Ok((id, Self {
            name: map.remove_string("name")?,
            // version: map.remove_string("version")?,
            dependencies: map
                .remove_array("dependencies")?
                .into_iter()
                .map(Dependency::from_value)
                .collect::<Result<_, _>>()?,
            features: map
                .remove_object("features")?
                .into_iter()
                .map(|(k, v)| {
                    into_array(v)
                        .and_then(|v| v.into_iter().map(into_string).collect::<Option<_>>())
                        .map(|v| (k, v))
                })
                .collect::<Option<_>>()
                .ok_or("features")?,
            manifest_path: map.remove_string("manifest_path")?,
            // This field was added in Rust 1.39.
            publish: if cargo_version >= 39 {
                // Publishing is unrestricted if null, and forbidden if an empty array.
                map.remove_nullable("publish", into_array)?.map_or(true, |a| !a.is_empty())
            } else {
                true
            },
            // This field was added in Rust 1.58.
            rust_version: if cargo_version >= 58 {
                map.remove_nullable("rust_version", into_string)?
            } else {
                None
            },
        }))
    }

    pub(crate) fn optional_deps(&self) -> impl Iterator<Item = &str> + '_ {
        self.dependencies.iter().filter_map(Dependency::as_feature)
    }
}

/// A dependency of the main crate.
pub(crate) struct Dependency {
    /// The name of the dependency.
    pub(crate) name: String,
    // /// The version requirement for the dependency.
    // pub(crate) req: String,
    /// Whether or not this is an optional dependency.
    pub(crate) optional: bool,
    // TODO: support this
    // /// The target platform for the dependency.
    // /// This is `None` if it is not a target dependency.
    // pub(crate) target: Option<String>,
    /// If the dependency is renamed, this is the new name for the dependency
    /// as a string.
    /// This is `None` if it is not renamed.
    ///
    /// This is always `None` if running with a version of Cargo older than 1.26.
    pub(crate) rename: Option<String>,
}

impl Dependency {
    fn from_value(mut value: Value) -> ParseResult<Self> {
        let map = value.as_object_mut().ok_or("dependencies")?;

        Ok(Self {
            name: map.remove_string("name")?,
            // req: map.remove_string("req")?,
            optional: map.get("optional").and_then(Value::as_bool).ok_or("optional")?,
            // This field was added in Rust 1.26.
            rename: map.remove_nullable("rename", into_string)?,
        })
    }

    pub(crate) fn as_feature(&self) -> Option<&str> {
        if self.optional {
            Some(self.rename.as_ref().unwrap_or(&self.name))
        } else {
            None
        }
    }
}

#[allow(clippy::option_option)]
fn allow_null<T>(value: Value, f: impl FnOnce(Value) -> Option<T>) -> Option<Option<T>> {
    if value.is_null() {
        Some(None)
    } else {
        f(value).map(Some)
    }
}

fn into_string<S: From<String>>(value: Value) -> Option<S> {
    if let Value::String(string) = value {
        Some(string.into())
    } else {
        None
    }
}

fn into_array(value: Value) -> Option<Vec<Value>> {
    if let Value::Array(array) = value {
        Some(array)
    } else {
        None
    }
}

fn into_object(value: Value) -> Option<Object> {
    if let Value::Object(object) = value {
        Some(object)
    } else {
        None
    }
}

trait ObjectExt {
    fn remove_string<S: From<String>>(&mut self, key: &'static str) -> ParseResult<S>;
    fn remove_array(&mut self, key: &'static str) -> ParseResult<Vec<Value>>;
    fn remove_object(&mut self, key: &'static str) -> ParseResult<Object>;
    fn remove_nullable<T>(
        &mut self,
        key: &'static str,
        f: impl FnOnce(Value) -> Option<T>,
    ) -> ParseResult<Option<T>>;
}

impl ObjectExt for Object {
    fn remove_string<S: From<String>>(&mut self, key: &'static str) -> ParseResult<S> {
        self.remove(key).and_then(into_string).ok_or(key)
    }
    fn remove_array(&mut self, key: &'static str) -> ParseResult<Vec<Value>> {
        self.remove(key).and_then(into_array).ok_or(key)
    }
    fn remove_object(&mut self, key: &'static str) -> ParseResult<Object> {
        self.remove(key).and_then(into_object).ok_or(key)
    }
    fn remove_nullable<T>(
        &mut self,
        key: &'static str,
        f: impl FnOnce(Value) -> Option<T>,
    ) -> ParseResult<Option<T>> {
        self.remove(key).and_then(|v| allow_null(v, f)).ok_or(key)
    }
}
