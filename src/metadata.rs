use anyhow::{format_err, Context as _};
use serde_json::{Map, Value};
use std::{
    borrow::Cow,
    collections::{BTreeSet, HashMap},
    ffi::OsStr,
    fmt,
    io::{self, Write},
    path::PathBuf,
    process::Command,
    vec,
};

use crate::{cli::Args, Context, Result};

type Object = Map<String, Value>;

// Refs:
// * https://github.com/rust-lang/cargo/blob/0.44.0/src/cargo/ops/cargo_output_metadata.rs#L56-L63
// * https://github.com/rust-lang/cargo/blob/0.44.0/src/cargo/core/package.rs#L57-L80
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
    // /// Nodes in a dependencies graph
    // pub(crate) nodes: HashMap<PackageId, Node>,
    /// Workspace root
    pub(crate) workspace_root: PathBuf,
}

impl Metadata {
    pub(crate) fn new(args: &Args<'_>, cargo: &OsStr) -> Result<Self> {
        let mut command = Command::new(cargo);
        command.args(&["metadata", "--no-deps", "--format-version=1"]);
        if let Some(manifest_path) = &args.manifest_path {
            command.arg("--manifest-path");
            command.arg(manifest_path);
        }

        let output = command.output().context("failed to run 'cargo metadata'")?;
        if !output.status.success() {
            let _ = io::stderr().write_all(&output.stderr);
            let code = output.status.code().unwrap_or(1);
            std::process::exit(code);
        }

        let value = serde_json::from_slice(&output.stdout).context("failed to parse metadata")?;
        Self::from_value(value).ok_or_else(|| format_err!("failed to parse metadata"))
    }

    fn from_value(mut value: Value) -> Option<Self> {
        let map = value.as_object_mut()?;

        Some(Self {
            packages: map
                .remove_array("packages")?
                .map(Package::from_value)
                .collect::<Option<_>>()?,
            workspace_members: map
                .remove_array("workspace_members")?
                .map(|v| into_string(v).map(PackageId::new))
                .collect::<Option<_>>()?,
            // nodes: match map.get_mut("resolve") {
            //     Some(value) => {
            //         let map = value.as_object_mut()?;
            //         map.remove_array("nodes")?.map(Node::from_value).collect::<Option<_>>()?
            //     }
            //     None => HashMap::new(),
            // },
            workspace_root: map.remove_string("workspace_root")?.into(),
        })
    }
}

// /// A node in a dependencies graph
// pub(crate) struct Node {
//     /// An opaque identifier for a package
//     pub(crate) id: PackageId,
//     /// Dependencies in a structured format.
//     pub(crate) deps: Vec<NodeDep>,
//     /// Features enabled on the crate
//     pub(crate) features: Vec<String>,
// }

// impl Node {
//     fn from_value(mut value: Value) -> Option<(PackageId, Self)> {
//         let map = value.as_object_mut()?;

//         let this = Self {
//             id: PackageId::new(map.remove_string("id")?),
//             deps: map.remove_array("deps")?.map(NodeDep::from_value).collect::<Option<_>>()?,
//             features: map.remove_array("features")?.map(into_string).collect::<Option<_>>()?,
//         };
//         Some((this.id.clone(), this))
//     }
// }

// /// A dependency in a node
// pub(crate) struct NodeDep {
//     /// Package ID (opaque unique identifier)
//     pub(crate) pkg: PackageId,
//     /// The kinds of dependencies.
//     ///
//     /// This field was added in Rust 1.41.
//     pub(crate) dep_kinds: Vec<DepKindInfo>,
// }

// impl NodeDep {
//     fn from_value(mut value: Value) -> Option<Self> {
//         let map = value.as_object_mut()?;

//         Some(Self {
//             pkg: PackageId::new(map.remove_string("pkg")?),
//             dep_kinds: map
//                 .remove_array("dep_kinds")?
//                 .map(DepKindInfo::from_value)
//                 .collect::<Option<_>>()?,
//         })
//     }
// }

// /// Information about a dependency kind.
// pub(crate) struct DepKindInfo {
//     /// The kind of dependency.
//     pub(crate) kind: Option<String>,
//     /// The target platform for the dependency.
//     ///
//     /// This is `None` if it is not a target dependency.
//     ///
//     /// By default all platform dependencies are included in the resolve
//     /// graph. Use Cargo's `--filter-platform` flag if you only want to
//     /// include dependencies for a specific platform.
//     pub(crate) target: Option<Platform>,
// }

// impl DepKindInfo {
//     fn from_value(mut value: Value) -> Option<Self> {
//         let map = value.as_object_mut()?;

//         Some(Self {
//             kind: allow_null(map.remove("kind")?, into_string)?,
//             target: allow_null(map.remove("target")?, into_string)?,
//         })
//     }
// }

// type Platform = String;

pub(crate) struct Package {
    /// Name as given in the `Cargo.toml`
    pub(crate) name: String,
    // /// Version given in the `Cargo.toml`
    // pub(crate) version: String,
    /// An opaque identifier for a package
    pub(crate) id: PackageId,
    /// List of dependencies of this particular package
    pub(crate) dependencies: Vec<Dependency>,
    /// Features provided by the crate, mapped to the features required by that feature.
    pub(crate) features: BTreeSet<String>,
    /// Path containing the `Cargo.toml`
    pub(crate) manifest_path: PathBuf,
}

impl Package {
    fn from_value(mut value: Value) -> Option<(PackageId, Self)> {
        let map = value.as_object_mut()?;

        let this = Self {
            name: map.remove_string("name")?,
            // version: map.remove_string("version")?,
            id: PackageId::new(map.remove_string("id")?),
            dependencies: map
                .remove_array("dependencies")?
                .map(Dependency::from_value)
                .collect::<Option<_>>()?,
            // Check if values are array, but don't collect because we don't use them.
            features: map
                .remove_object("features")?
                .into_iter()
                .map(|(k, v)| v.as_array().map(|_| k))
                .collect::<Option<_>>()?,
            manifest_path: map.remove_string("manifest_path")?.into(),
        };
        Some((this.id.clone(), this))
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
    pub(crate) rename: Option<String>,
}

impl Dependency {
    fn from_value(mut value: Value) -> Option<Self> {
        let map = value.as_object_mut()?;

        Some(Self {
            name: map.remove_string("name")?,
            // req: map.remove_string("req")?,
            optional: map.remove("optional")?.as_bool()?,
            rename: allow_null(map.remove("rename")?, into_string)?,
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

fn into_object(value: Value) -> Option<Object> {
    if let Value::Object(object) = value { Some(object) } else { None }
}

trait ObjectExt {
    fn remove_string(&mut self, key: &str) -> Option<String>;
    fn remove_array(&mut self, key: &str) -> Option<vec::IntoIter<Value>>;
    fn remove_object(&mut self, key: &str) -> Option<Object>;
}

impl ObjectExt for Object {
    fn remove_string(&mut self, key: &str) -> Option<String> {
        into_string(self.remove(key)?)
    }
    fn remove_array(&mut self, key: &str) -> Option<vec::IntoIter<Value>> {
        if let Value::Array(array) = self.remove(key)? { Some(array.into_iter()) } else { None }
    }
    fn remove_object(&mut self, key: &str) -> Option<Object> {
        into_object(self.remove(key)?)
    }
}
