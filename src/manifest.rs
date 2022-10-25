use std::{collections::BTreeMap, path::Path};

use anyhow::{format_err, Context as _, Result};

use crate::{fs, metadata::Metadata};

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

    pub(crate) fn remove_dev_deps(&self) -> String {
        let mut doc = self.doc.clone();
        remove_dev_deps(&mut doc);
        doc.to_string()
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
[dev-dependencies.opencl]
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
[dev-dependencies.opencl]
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

[dev-dependencies.opencl]


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
