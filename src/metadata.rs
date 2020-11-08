use anyhow::{format_err, Context as _};
use serde_json::{Map, Value};
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    fmt,
    path::PathBuf,
    rc::Rc,
};

use crate::{cli::Args, Cargo, Context, Result};

type Object = Map<String, Value>;
type ParseResult<T> = Result<T, &'static str>;

// Refs:
// * https://github.com/rust-lang/cargo/blob/0.47.0/src/cargo/ops/cargo_output_metadata.rs#L56-L63
// * https://github.com/rust-lang/cargo/blob/0.47.0/src/cargo/core/package.rs#L57-L80
// * https://github.com/oli-obk/cargo_metadata

/// An "opaque" identifier for a package.
/// It is possible to inspect the `repr` field, if the need arises, but its
/// precise format is an implementation detail and is subject to change.
#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub(crate) struct PackageId {
    /// The underlying string representation of id.
    repr: Rc<str>,
}

impl PackageId {
    fn new(repr: String) -> Self {
        Self { repr: repr.into() }
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.repr, f)
    }
}

pub(crate) struct Metadata {
    /// A list of all crates referenced by this crate (and the crate itself)
    pub(crate) packages: HashMap<PackageId, Package>,
    /// A list of all workspace members
    pub(crate) workspace_members: Vec<PackageId>,
    /// Dependencies graph
    pub(crate) resolve: Resolve,
    /// Workspace root
    pub(crate) workspace_root: PathBuf,
}

impl Metadata {
    pub(crate) fn new(args: &Args<'_>, cargo: &Cargo) -> Result<Self> {
        let mut command = cargo.process();
        command.args(&["metadata", "--format-version=1"]);
        if let Some(manifest_path) = &args.manifest_path {
            command.arg("--manifest-path");
            command.arg(manifest_path);
        }
        let output = command.exec_with_output()?;

        let map =
            serde_json::from_slice(&output.stdout).context("failed to parse metadata as json")?;
        Self::from_obj(map, cargo)
            .map_err(|s| format_err!("failed to parse `{}` field from metadata", s))
    }

    fn from_obj(mut map: Object, cargo: &Cargo) -> ParseResult<Self> {
        let workspace_members: Vec<_> = map
            .remove_array("workspace_members")?
            .into_iter()
            .map(|v| into_string(v).map(PackageId::new).ok_or("workspace_members"))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            packages: map
                .remove_array("packages")?
                .into_iter()
                .map(|v| Package::from_value(v, cargo))
                .collect::<Result<_, _>>()?,
            workspace_members,
            resolve: Resolve::from_obj(map.remove_object("resolve")?, cargo)?,
            workspace_root: map.remove_string("workspace_root")?.into(),
        })
    }
}

/// A dependency graph
pub(crate) struct Resolve {
    /// Nodes in a dependencies graph
    pub(crate) nodes: HashMap<PackageId, Node>,
    // if `None`, cargo-hack called in the root of a virtual workspace
    /// The crate for which the metadata was read.
    pub(crate) root: Option<PackageId>,
}

impl Resolve {
    fn from_obj(mut map: Object, cargo: &Cargo) -> ParseResult<Self> {
        Ok(Self {
            nodes: map
                .remove_array("nodes")?
                .into_iter()
                .map(|v| Node::from_value(v, cargo))
                .collect::<Result<_, _>>()?,
            root: map.remove_nullable("root", into_string)?.map(PackageId::new),
        })
    }
}

/// A node in a dependencies graph
pub(crate) struct Node {
    /// Dependencies in a structured format.
    ///
    /// This is always empty if running with a version of Cargo older than 1.30.
    pub(crate) deps: Vec<NodeDep>,
}

impl Node {
    fn from_value(mut value: Value, cargo: &Cargo) -> ParseResult<(PackageId, Self)> {
        let map = value.as_object_mut().ok_or("nodes")?;

        let id = map.remove_string("id").map(PackageId::new)?;
        Ok((id, Self {
            // This field was added in Rust 1.30.
            deps: if cargo.version >= 30 {
                map.remove_array("deps")?
                    .into_iter()
                    .map(|v| NodeDep::from_value(v, cargo))
                    .collect::<Result<_, _>>()?
            } else {
                Vec::new()
            },
        }))
    }
}

/// A dependency in a node
pub(crate) struct NodeDep {
    /// Package ID (opaque unique identifier)
    pub(crate) pkg: PackageId,
    /// The kinds of dependencies.
    ///
    /// This is always empty if running with a version of Cargo older than 1.41.
    pub(crate) dep_kinds: Vec<DepKindInfo>,
}

impl NodeDep {
    fn from_value(mut value: Value, cargo: &Cargo) -> ParseResult<Self> {
        let map = value.as_object_mut().ok_or("deps")?;

        Ok(Self {
            pkg: PackageId::new(map.remove_string("pkg")?),
            // This field was added in Rust 1.41.
            dep_kinds: if cargo.version >= 41 {
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
    ///
    /// This is `None` if it is not a target dependency.
    ///
    /// By default all platform dependencies are included in the resolve
    /// graph. Use Cargo's `--filter-platform` flag if you only want to
    /// include dependencies for a specific platform.
    pub(crate) target: Option<Platform>,
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

type Platform = String;

pub(crate) struct Package {
    /// Name as given in the `Cargo.toml`
    pub(crate) name: String,
    // /// Version given in the `Cargo.toml`
    // pub(crate) version: String,
    /// List of dependencies of this particular package
    pub(crate) dependencies: Vec<Dependency>,
    /// Features provided by the crate, mapped to the features required by that feature.
    pub(crate) features: BTreeMap<String, Vec<String>>,
    /// Path containing the `Cargo.toml`
    pub(crate) manifest_path: PathBuf,
    /// List of registries to which this package may be published.
    ///
    /// This is always `true` if running with a version of Cargo older than 1.39.
    pub(crate) publish: bool,
}

impl Package {
    fn from_value(mut value: Value, cargo: &Cargo) -> ParseResult<(PackageId, Self)> {
        let map = value.as_object_mut().ok_or("packages")?;

        let id = PackageId::new(map.remove_string("id")?);
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
            manifest_path: map.remove_string("manifest_path")?.into(),
            // This field was added in Rust 1.39.
            publish: if cargo.version >= 39 {
                // Publishing is unrestricted if `None`, and forbidden if the `Vec` is empty.
                map.remove_nullable("publish", into_array)?.map_or(true, |a| !a.is_empty())
            } else {
                true
            },
        }))
    }

    pub(crate) fn name_verbose(&self, cx: &Context<'_>) -> Cow<'_, str> {
        if cx.verbose {
            Cow::Owned(format!(
                "{} ({})",
                self.name,
                self.manifest_path.parent().unwrap().display()
            ))
        } else {
            Cow::Borrowed(&self.name)
        }
    }
}

/// A dependency of the main crate
pub(crate) struct Dependency {
    /// Name as given in the `Cargo.toml`
    pub(crate) name: String,
    // /// The required version
    // pub(crate) req: String,
    /// Whether this dependency is required or optional
    pub(crate) optional: bool,
    // TODO: support this
    // /// The target this dependency is specific to.
    // pub(crate) target: Option<String>,
    /// If the dependency is renamed, this is the new name for the dependency
    /// as a string.  None if it is not renamed.
    ///
    /// This field was added in Rust 1.26.
    pub(crate) rename: Option<String>,
}

impl Dependency {
    fn from_value(mut value: Value) -> ParseResult<Self> {
        let map = value.as_object_mut().ok_or("dependencies")?;

        Ok(Self {
            name: map.remove_string("name")?,
            // req: map.remove_string("req")?,
            optional: map.get("optional").and_then(Value::as_bool).ok_or("optional")?,
            rename: map.remove_nullable("rename", into_string)?,
        })
    }

    pub(crate) fn as_feature(&self) -> Option<&str> {
        if self.optional { Some(self.rename.as_ref().unwrap_or(&self.name)) } else { None }
    }
}

fn allow_null<T>(value: Value, f: impl FnOnce(Value) -> Option<T>) -> Option<Option<T>> {
    if value.is_null() { Some(None) } else { f(value).map(Some) }
}

fn into_string(value: Value) -> Option<String> {
    if let Value::String(string) = value { Some(string) } else { None }
}

fn into_array(value: Value) -> Option<Vec<Value>> {
    if let Value::Array(array) = value { Some(array) } else { None }
}

fn into_object(value: Value) -> Option<Object> {
    if let Value::Object(object) = value { Some(object) } else { None }
}

trait ObjectExt {
    fn remove_string<'a>(&mut self, key: &'a str) -> Result<String, &'a str>;
    fn remove_array<'a>(&mut self, key: &'a str) -> Result<Vec<Value>, &'a str>;
    fn remove_object<'a>(&mut self, key: &'a str) -> Result<Object, &'a str>;
    fn remove_nullable<'a, T>(
        &mut self,
        key: &'a str,
        f: impl FnOnce(Value) -> Option<T>,
    ) -> Result<Option<T>, &'a str>;
}

impl ObjectExt for Object {
    fn remove_string<'a>(&mut self, key: &'a str) -> Result<String, &'a str> {
        self.remove(key).and_then(into_string).ok_or(key)
    }
    fn remove_array<'a>(&mut self, key: &'a str) -> Result<Vec<Value>, &'a str> {
        self.remove(key).and_then(into_array).ok_or(key)
    }
    fn remove_object<'a>(&mut self, key: &'a str) -> Result<Object, &'a str> {
        self.remove(key).and_then(into_object).ok_or(key)
    }
    fn remove_nullable<'a, T>(
        &mut self,
        key: &'a str,
        f: impl FnOnce(Value) -> Option<T>,
    ) -> Result<Option<T>, &'a str> {
        self.remove(key).and_then(|v| allow_null(v, f)).ok_or(key)
    }
}
