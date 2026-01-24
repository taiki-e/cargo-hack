// SPDX-License-Identifier: Apache-2.0 OR MIT

// Refs:
// - https://doc.rust-lang.org/nightly/cargo/commands/cargo-metadata.html#output-format
// - https://github.com/rust-lang/cargo/blob/0.47.0/src/cargo/ops/cargo_output_metadata.rs#L56-L63
// - https://github.com/rust-lang/cargo/blob/0.47.0/src/cargo/core/package.rs#L57-L80
// - https://github.com/oli-obk/cargo_metadata

use std::{
    collections::{BTreeMap, HashMap},
    ffi::OsStr,
    ops,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, format_err};
use cargo_config2::Config;
use serde_json::{Map, Value};

use crate::{cargo, cli::Args, fs, process::ProcessBuilder, restore, term};

type Object = Map<String, Value>;
type ParseResult<T> = Result<T, &'static str>;

/// An opaque unique identifier for referring to the package.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub(crate) struct PackageId {
    index: usize,
}

pub(crate) struct Metadata {
    pub(crate) cargo_version: u32,
    /// List of all packages in the workspace and all feature-enabled dependencies.
    //
    /// This doesn't contain dependencies if cargo-metadata is run with --no-deps.
    pub(crate) packages: Box<[Package]>,
    /// List of members of the workspace.
    pub(crate) workspace_members: Vec<PackageId>,
    /// The resolved dependency graph for the entire workspace.
    pub(crate) resolve: Resolve,
    /// The absolute path to the root of the workspace.
    pub(crate) workspace_root: PathBuf,
}

impl Metadata {
    pub(crate) fn new(
        manifest_path: Option<&str>,
        cargo: &OsStr,
        mut cargo_version: u32,
        args: &Args,
        restore: &mut restore::Manager,
    ) -> Result<Self> {
        let stable_cargo_version =
            cargo::version(cmd!("rustup", "run", "stable", "cargo")).map(|v| v.minor).unwrap_or(0);

        let config;
        let include_deps_features = if args.include_deps_features {
            config = Config::load()?;
            let targets = config.build_target_for_cli(&args.target)?;
            let host = config.host_triple()?;
            Some((targets, host))
        } else {
            None
        };

        let mut cmd;
        let append_metadata_args = |cmd: &mut ProcessBuilder<'_>| {
            cmd.arg("metadata");
            cmd.arg("--format-version=1");
            if let Some(manifest_path) = manifest_path {
                cmd.arg("--manifest-path");
                cmd.arg(manifest_path);
            }
            if let Some((targets, host)) = &include_deps_features {
                if targets.is_empty() {
                    cmd.arg("--filter-platform");
                    cmd.arg(host);
                } else {
                    for target in targets {
                        cmd.arg("--filter-platform");
                        cmd.arg(target);
                    }
                }
                // features-related flags are unneeded when --no-deps is used.
                // TODO:
                // cmd.arg("--all-features");
            } else {
                cmd.arg("--no-deps");
            }
        };
        let json = if stable_cargo_version > cargo_version {
            cmd = cmd!(cargo, "metadata", "--format-version=1", "--no-deps");
            if let Some(manifest_path) = manifest_path {
                cmd.arg("--manifest-path");
                cmd.arg(manifest_path);
            }
            let no_deps_raw = cmd.read()?;
            let no_deps: Object = serde_json::from_str(&no_deps_raw)
                .with_context(|| format!("failed to parse output from {cmd}"))?;
            let lockfile =
                Path::new(no_deps["workspace_root"].as_str().unwrap()).join("Cargo.lock");
            if !lockfile.exists() {
                let mut cmd = cmd!(cargo, "generate-lockfile");
                if let Some(manifest_path) = manifest_path {
                    cmd.arg("--manifest-path");
                    cmd.arg(manifest_path);
                }
                cmd.run_with_output()?;
            }
            let guard = term::verbose::scoped(false);
            restore.register_always(fs::read(&lockfile)?, lockfile);
            // Try with stable cargo because if workspace member has
            // a dependency that requires newer cargo features, `cargo metadata`
            // with older cargo may fail.
            cmd = cmd!("rustup", "run", "stable", "cargo");
            append_metadata_args(&mut cmd);
            let json = cmd.read();
            restore.restore_last()?;
            drop(guard);
            match json {
                Ok(json) => {
                    cargo_version = stable_cargo_version;
                    json
                }
                Err(_e) => {
                    if include_deps_features.is_some() {
                        // If failed, try again with the version of cargo we will actually use.
                        cmd = cmd!(cargo);
                        append_metadata_args(&mut cmd);
                        cmd.read()?
                    } else {
                        no_deps_raw
                    }
                }
            }
        } else {
            cmd = cmd!(cargo);
            append_metadata_args(&mut cmd);
            cmd.read()?
        };

        let map = serde_json::from_str(&json)
            .with_context(|| format!("failed to parse output from {cmd}"))?;
        Self::from_obj(map, cargo_version)
            .map_err(|s| format_err!("failed to parse `{s}` field from metadata"))
    }

    fn from_obj(mut map: Object, cargo_version: u32) -> ParseResult<Self> {
        let raw_packages = map.remove_array("packages")?;
        let mut packages = Vec::with_capacity(raw_packages.len());
        let mut pkg_id_map = HashMap::with_capacity(raw_packages.len());
        for (i, pkg) in raw_packages.into_iter().enumerate() {
            let (id, pkg) = Package::from_value(pkg, cargo_version)?;
            pkg_id_map.insert(id, i);
            packages.push(pkg);
        }
        let workspace_members: Vec<_> = map
            .remove_array("workspace_members")?
            .into_iter()
            .map(|v| -> ParseResult<_> {
                let id: String = into_string(v).ok_or("workspace_members")?;
                Ok(PackageId { index: pkg_id_map[&id] })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            cargo_version,
            packages: packages.into_boxed_slice(),
            workspace_members,
            resolve: match map.remove_nullable("resolve", into_object)? {
                Some(resolve) => Resolve::from_obj(resolve, &pkg_id_map, cargo_version)?,
                None => Resolve { nodes: HashMap::default() },
            },
            workspace_root: map.remove_string("workspace_root")?,
        })
    }
}

impl ops::Index<PackageId> for Metadata {
    type Output = Package;
    #[inline]
    fn index(&self, index: PackageId) -> &Self::Output {
        &self.packages[index.index]
    }
}

/// The resolved dependency graph for the entire workspace.
pub(crate) struct Resolve {
    /// Nodes in a dependency graph.
    ///
    /// This is always empty if cargo-metadata is run with --no-deps.
    pub(crate) nodes: HashMap<PackageId, Node>,
}

impl Resolve {
    fn from_obj(
        mut map: Object,
        pkg_id_map: &HashMap<String, usize>,
        cargo_version: u32,
    ) -> ParseResult<Self> {
        let nodes = map
            .remove_array("nodes")?
            .into_iter()
            .map(|v| -> ParseResult<_> {
                let (id, node) = Node::from_value(v, pkg_id_map, cargo_version)?;
                Ok((PackageId { index: pkg_id_map[&id] }, node))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { nodes })
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
    fn from_value(
        mut value: Value,
        pkg_id_map: &HashMap<String, usize>,
        cargo_version: u32,
    ) -> ParseResult<(String, Self)> {
        let map = value.as_object_mut().ok_or("nodes")?;

        let id = map.remove_string("id")?;
        Ok((id, Self {
            // This field was added in Rust 1.30.
            deps: if cargo_version >= 30 {
                map.remove_array("deps")?
                    .into_iter()
                    .map(|v| NodeDep::from_value(v, pkg_id_map, cargo_version))
                    .collect::<Result<_, _>>()?
            } else {
                vec![]
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
    fn from_value(
        mut value: Value,
        pkg_id_map: &HashMap<String, usize>,
        cargo_version: u32,
    ) -> ParseResult<Self> {
        let map = value.as_object_mut().ok_or("deps")?;

        let id: String = map.remove_string("pkg")?;
        Ok(Self {
            pkg: PackageId { index: pkg_id_map[&id] },
            // This field was added in Rust 1.41.
            dep_kinds: if cargo_version >= 41 {
                map.remove_array("dep_kinds")?
                    .into_iter()
                    .map(DepKindInfo::from_value)
                    .collect::<Result<_, _>>()?
            } else {
                vec![]
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
    pub(crate) rust_version: Option<String>,
}

impl Package {
    fn from_value(mut value: Value, cargo_version: u32) -> ParseResult<(String, Self)> {
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
                map.remove_nullable("publish", into_array)?.is_none_or(|a| !a.is_empty())
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
        if self.optional { Some(self.rename.as_ref().unwrap_or(&self.name)) } else { None }
    }
}

#[allow(clippy::option_option)]
fn allow_null<T>(value: Value, f: impl FnOnce(Value) -> Option<T>) -> Option<Option<T>> {
    if value.is_null() { Some(None) } else { f(value).map(Some) }
}

fn into_string<S: From<String>>(value: Value) -> Option<S> {
    if let Value::String(string) = value { Some(string.into()) } else { None }
}
fn into_array(value: Value) -> Option<Vec<Value>> {
    if let Value::Array(array) = value { Some(array) } else { None }
}
fn into_object(value: Value) -> Option<Object> {
    if let Value::Object(object) = value { Some(object) } else { None }
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
