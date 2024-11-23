// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use anyhow::{bail, format_err, Context as _, Result};

use crate::{context::Context, fs, term};

type ParseResult<T> = Result<T, &'static str>;

// Cargo manifest
// https://doc.rust-lang.org/nightly/cargo/reference/manifest.html
pub(crate) struct Manifest {
    raw: String,
    doc: toml_edit::DocumentMut,
    pub(crate) package: Package,
    pub(crate) features: BTreeMap<String, Vec<String>>,
}

impl Manifest {
    pub(crate) fn new(path: &Path, metadata_cargo_version: u32) -> Result<Self> {
        let raw = fs::read_to_string(path)?;
        let doc: toml_edit::DocumentMut = raw
            .parse()
            .with_context(|| format!("failed to parse manifest `{}` as toml", path.display()))?;
        let package = Package::from_table(&doc, metadata_cargo_version).map_err(|s| {
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
    fn from_table(doc: &toml_edit::DocumentMut, metadata_cargo_version: u32) -> ParseResult<Self> {
        let package = doc.get("package").and_then(toml_edit::Item::as_table).ok_or("package")?;

        Ok(Self {
            // Publishing is unrestricted if `true` or the field is not
            // specified, and forbidden if `false` or the array is empty.
            publish: if metadata_cargo_version >= 39 {
                None // Use `metadata.package.publish` instead.
            } else {
                Some(match package.get("publish") {
                    None => true,
                    Some(toml_edit::Item::Value(toml_edit::Value::Boolean(b))) => *b.value(),
                    Some(toml_edit::Item::Value(toml_edit::Value::Array(a))) => !a.is_empty(),
                    Some(_) => return Err("publish"),
                })
            },
            rust_version: if metadata_cargo_version >= 58 {
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
    fn from_table(doc: &toml_edit::DocumentMut) -> ParseResult<BTreeMap<String, Vec<String>>> {
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
    if no_dev_deps || no_private {
        let workspace_root = &cx.metadata.workspace_root;
        let root_manifest = &workspace_root.join("Cargo.toml");
        let mut root_id = None;
        let mut private_crates = BTreeSet::new();
        for id in &cx.metadata.workspace_members {
            let package = cx.packages(id);
            let manifest_path = &*package.manifest_path;
            let is_root = manifest_path == root_manifest;
            if is_root {
                root_id = Some(id);
            }
            let is_private = cx.is_private(id);
            if is_private && no_private {
                if is_root {
                    bail!(
                        "--no-private is not supported yet with workspace with private root crate"
                    );
                }
                private_crates.insert(manifest_path);
            } else if is_root && no_private {
                // This case is handled in the if block after loop.
            } else if no_dev_deps {
                let manifest = cx.manifests(id);
                let mut doc = manifest.doc.clone();
                if term::verbose() {
                    info!("removing dev-dependencies from {}", manifest_path.display());
                }
                remove_dev_deps(&mut doc);
                cx.restore.register(&manifest.raw, manifest_path);
                fs::write(manifest_path, doc.to_string())?;
            }
        }
        if no_private && (no_dev_deps && root_id.is_some() || !private_crates.is_empty()) {
            let manifest_path = root_manifest;
            let (mut doc, orig) = match root_id {
                Some(id) => {
                    let manifest = cx.manifests(id);
                    (manifest.doc.clone(), manifest.raw.clone())
                }
                None => {
                    let orig = fs::read_to_string(manifest_path)?;
                    (
                        orig.parse().with_context(|| {
                            format!(
                                "failed to parse manifest `{}` as toml",
                                manifest_path.display()
                            )
                        })?,
                        orig,
                    )
                }
            };
            if no_dev_deps && root_id.is_some() {
                if term::verbose() {
                    info!("removing dev-dependencies from {}", manifest_path.display());
                }
                remove_dev_deps(&mut doc);
            }
            if !private_crates.is_empty() {
                if term::verbose() {
                    info!("removing private crates from {}", manifest_path.display());
                }
                remove_private_crates(&mut doc, workspace_root, private_crates);
            }
            cx.restore.register(orig, manifest_path);
            fs::write(manifest_path, doc.to_string())?;
        }
        if restore_lockfile {
            let lockfile = &workspace_root.join("Cargo.lock");
            if lockfile.exists() {
                cx.restore.register(fs::read_to_string(lockfile)?, lockfile);
            }
        }
    }

    f()?;

    // Restore original Cargo.toml and Cargo.lock.
    cx.restore.restore_all();

    Ok(())
}

fn remove_dev_deps(doc: &mut toml_edit::DocumentMut) {
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
    doc: &mut toml_edit::DocumentMut,
    workspace_root: &Path,
    mut private_crates: BTreeSet<&Path>,
) {
    let table = doc.as_table_mut();
    if let Some(workspace) = table.get_mut("workspace").and_then(toml_edit::Item::as_table_like_mut)
    {
        if let Some(members) = workspace.get_mut("members").and_then(toml_edit::Item::as_array_mut)
        {
            let mut i = 0;
            while i < members.len() {
                if let Some(member) = members.get(i).and_then(toml_edit::Value::as_str) {
                    let manifest_path = workspace_root.join(member).join("Cargo.toml");
                    if let Some(p) = private_crates.iter().find_map(|p| {
                        same_file::is_same_file(p, &manifest_path).ok().and_then(|v| {
                            if v {
                                Some(*p)
                            } else {
                                None
                            }
                        })
                    }) {
                        members.remove(i);
                        private_crates.remove(p);
                        continue;
                    }
                }
                i += 1;
            }
        }
        if private_crates.is_empty() {
            return;
        }
        // Handles the case that the members field contains glob.
        // TODO: test that it also works when public and private crates are nested.
        if let Some(exclude) = workspace.get_mut("exclude").and_then(toml_edit::Item::as_array_mut)
        {
            for private_crate in private_crates {
                exclude.push(private_crate.parent().unwrap().to_str().unwrap());
            }
        } else {
            workspace.insert(
                "exclude",
                toml_edit::Item::Value(toml_edit::Value::Array(
                    private_crates
                        .iter()
                        .map(|p| {
                            toml_edit::Value::String(toml_edit::Formatted::new(
                                p.parent().unwrap().to_str().unwrap().to_owned(),
                            ))
                        })
                        .collect::<toml_edit::Array>(),
                )),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::remove_dev_deps;

    macro_rules! test {
        ($name:ident, $input:expr, $expected:expr) => {
            #[test]
            fn $name() {
                let mut doc: toml_edit::DocumentMut = $input.parse().unwrap();
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
