use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{bail, format_err, Context as _, Result};

use crate::{
    context::Context,
    fs,
    metadata::{self, Metadata},
    term,
};

type ParseResult<T> = Result<T, &'static str>;

// Cargo manifest
// https://doc.rust-lang.org/nightly/cargo/reference/manifest.html
pub(crate) struct Manifest {
    pub(crate) raw: String,
    pub(crate) doc: toml_edit::Document,
    pub(crate) package: Package,
    pub(crate) features: BTreeMap<String, Vec<String>>,
}

impl Manifest {
    pub(crate) fn new(path: &Path, metadata: &Metadata) -> Result<Self> {
        let raw = fs::read_to_string(path)?;
        let doc: toml_edit::Document = raw
            .parse()
            .with_context(|| format!("failed to parse manifest `{}` as toml", path.display()))?;
        let package = Package::from_table(&doc, metadata).map_err(|s| {
            format_err!("failed to parse `{s}` field from manifest `{}`", path.display())
        })?;
        let features = Features::from_table(&doc).map_err(|s| {
            format_err!("failed to parse `{s}` field from manifest `{}`", path.display())
        })?;
        Ok(Self { raw, doc, package, features })
    }
}

pub(crate) struct Package {
    // `metadata.package.publish` requires Rust 1.39
    pub(crate) publish: Option<bool>,
    // `metadata.package.rust_version` requires Rust 1.58
    #[allow(clippy::option_option)]
    pub(crate) rust_version: Option<Option<String>>,
}

impl Package {
    fn from_table(doc: &toml_edit::Document, metadata: &Metadata) -> ParseResult<Self> {
        let package = doc.get("package").and_then(toml_edit::Item::as_table).ok_or("package")?;

        Ok(Self {
            // Publishing is unrestricted if `true` or the field is not
            // specified, and forbidden if `false` or the array is empty.
            publish: if metadata.cargo_version >= 39 {
                None // Use `metadata.package.publish` instead.
            } else {
                Some(match package.get("publish") {
                    None => true,
                    Some(toml_edit::Item::Value(toml_edit::Value::Boolean(b))) => *b.value(),
                    Some(toml_edit::Item::Value(toml_edit::Value::Array(a))) => !a.is_empty(),
                    Some(_) => return Err("publish"),
                })
            },
            rust_version: if metadata.cargo_version >= 58 {
                None // use `metadata.package.rust_version` instead.
            } else {
                Some(match package.get("rust-version").map(toml_edit::Item::as_str) {
                    None => None,
                    Some(Some(v)) => Some(v.to_owned()),
                    Some(None) => return Err("rust-version"),
                })
            },
        })
    }
}

struct Features {}

impl Features {
    fn from_table(doc: &toml_edit::Document) -> ParseResult<BTreeMap<String, Vec<String>>> {
        let features = match doc.get("features") {
            Some(features) => features.as_table().ok_or("features")?,
            None => return Ok(BTreeMap::new()),
        };
        let mut res = BTreeMap::new();
        for (name, values) in features {
            res.insert(
                name.to_owned(),
                values
                    .as_array()
                    .ok_or("features")?
                    .into_iter()
                    .filter_map(toml_edit::Value::as_str)
                    .map(str::to_owned)
                    .collect(),
            );
        }
        Ok(res)
    }
}

pub(crate) fn with(cx: &Context, f: impl FnOnce() -> Result<()>) -> Result<()> {
    // TODO: provide option to keep updated Cargo.lock
    let restore_lockfile = true;
    let no_dev_deps = cx.no_dev_deps | cx.remove_dev_deps;
    let no_private = cx.no_private;
    let restore_handles = if no_dev_deps || no_private {
        let mut restore_handles = Vec::with_capacity(cx.metadata.workspace_members.len());
        let workspace_root = &cx.metadata.workspace_root;
        let root_manifest = &workspace_root.join("Cargo.toml");
        let mut has_root_crate = false;
        let mut root_id = None;
        let mut private_crates = vec![];
        for id in &cx.metadata.workspace_members {
            let package = cx.packages(id);
            let manifest_path = &package.manifest_path;
            let is_root = manifest_path == root_manifest;
            if is_root {
                root_id = Some(id);
            }
            has_root_crate |= is_root;
            let is_private = cx.is_private(id);
            if is_private && no_private {
                if is_root {
                    bail!(
                        "--no-private is not supported yet with workspace with private root crate"
                    );
                }
                private_crates.push(manifest_path);
            } else if is_root && no_private {
                //
            } else if no_dev_deps {
                let manifest = cx.manifests(id);
                let mut doc = manifest.doc.clone();
                if term::verbose() {
                    info!("removing dev-dependencies from {}", manifest_path.display());
                }
                remove_dev_deps(&mut doc);
                restore_handles.push(cx.restore.register(&manifest.raw, manifest_path));
                fs::write(manifest_path, doc.to_string())?;
            }
        }
        if no_private && (no_dev_deps && has_root_crate || !private_crates.is_empty()) {
            let manifest_path = root_manifest;
            let manifest = cx.manifests(root_id.unwrap());
            let mut doc = manifest.doc.clone();
            if no_dev_deps && has_root_crate {
                if term::verbose() {
                    info!("removing dev-dependencies from {}", manifest_path.display());
                }
                remove_dev_deps(&mut doc);
            }
            if !private_crates.is_empty() {
                if term::verbose() {
                    info!("removing private crates from {}", manifest_path.display());
                }
                remove_private_crates(&mut doc, &cx.metadata, &private_crates)?;
            }
            restore_handles.push(cx.restore.register(&manifest.raw, manifest_path));
            fs::write(manifest_path, doc.to_string())?;
        }
        if restore_lockfile {
            let lockfile = &cx.metadata.workspace_root.join("Cargo.lock");
            if lockfile.exists() {
                restore_handles.push(cx.restore.register(fs::read_to_string(lockfile)?, lockfile));
            }
        }
        restore_handles
    } else {
        vec![]
    };

    f()?;

    // Restore original Cargo.toml and Cargo.lock.
    drop(restore_handles);

    Ok(())
}

fn remove_dev_deps(doc: &mut toml_edit::Document) {
    const KEY: &str = "dev-dependencies";
    let table = doc.as_table_mut();
    table.remove(KEY);
    if let Some(table) = table.get_mut("target").and_then(toml_edit::Item::as_table_like_mut) {
        for (_, val) in table.iter_mut() {
            if let Some(table) = val.as_table_like_mut() {
                table.remove(KEY);
            }
        }
    }
}

fn remove_private_crates(
    doc: &mut toml_edit::Document,
    metadata: &metadata::Metadata,
    private_crates: &[&PathBuf],
) -> Result<()> {
    let table = doc.as_table_mut();
    if let Some(workspace) = table.get_mut("workspace").and_then(toml_edit::Item::as_table_like_mut)
    {
        if let Some(members) = workspace.get_mut("members").and_then(toml_edit::Item::as_array_mut)
        {
            let mut i = 0;
            while i < members.len() {
                if let Some(member) = members.get(i).and_then(toml_edit::Value::as_str) {
                    let manifest_path =
                        metadata.workspace_root.join(member).join("Cargo.toml").canonicalize()?;
                    if private_crates.iter().any(|p| **p == manifest_path) {
                        members.remove(i);
                        continue;
                    }
                }
                i += 1;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::remove_dev_deps;

    macro_rules! test {
        ($name:ident, $input:expr, $expected:expr) => {
            #[test]
            fn $name() {
                let mut doc: toml_edit::Document = $input.parse().unwrap();
                remove_dev_deps(&mut doc);
                assert_eq!($expected, doc.to_string());
            }
        };
    }

    test!(
        a,
        "\
[package]
[dependencies]
[[example]]
[dev-dependencies.serde]
[dev-dependencies]",
        "\
[package]
[dependencies]
[[example]]
"
    );

    test!(
        b,
        "\
[package]
[dependencies]
[[example]]
[dev-dependencies.serde]
[dev-dependencies]
",
        "\
[package]
[dependencies]
[[example]]
"
    );

    test!(
        c,
        "\
[dev-dependencies]
foo = { features = [] }
bar = \"0.1\"
",
        "\
         "
    );

    test!(
        d,
        "\
[dev-dependencies.foo]
features = []

[dev-dependencies]
bar = { features = [], a = [] }

[dependencies]
bar = { features = [], a = [] }
",
        "
[dependencies]
bar = { features = [], a = [] }
"
    );

    test!(
        many_lines,
        "\
[package]\n\n

[dev-dependencies.serde]


[dev-dependencies]
",
        "\
[package]
"
    );

    test!(
        target_deps1,
        "\
[package]

[target.'cfg(unix)'.dev-dependencies]

[dependencies]
",
        "\
[package]

[dependencies]
"
    );

    test!(
        target_deps2,
        "\
[package]

[target.'cfg(unix)'.dev-dependencies]
foo = \"0.1\"

[target.'cfg(unix)'.dev-dependencies.bar]

[dev-dependencies]
foo = \"0.1\"

[target.'cfg(unix)'.dependencies]
foo = \"0.1\"
",
        "\
[package]

[target.'cfg(unix)'.dependencies]
foo = \"0.1\"
"
    );

    test!(
        target_deps3,
        "\
[package]

[target.'cfg(unix)'.dependencies]

[dev-dependencies]
",
        "\
[package]

[target.'cfg(unix)'.dependencies]
"
    );

    test!(
        target_deps4,
        "\
[package]

[target.'cfg(unix)'.dev-dependencies]
",
        "\
[package]
"
    );

    test!(
        not_table_multi_line,
        "\
[package]
foo = [
    ['dev-dependencies'],
    [\"dev-dependencies\"]
]
",
        "\
[package]
foo = [
    ['dev-dependencies'],
    [\"dev-dependencies\"]
]
"
    );
}
