// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, slice,
};

use crate::{manifest::Manifest, metadata::Metadata, PackageId};

#[derive(Debug)]
pub(crate) struct Features {
    features: Vec<Feature>,
    optional_deps_start: usize,
    deps_features_start: usize,
}

impl Features {
    pub(crate) fn new(
        metadata: &Metadata,
        manifest: &Manifest,
        id: &PackageId,
        include_deps_features: bool,
    ) -> Self {
        let package = &metadata.packages[id];

        let mut features: Vec<_> = manifest.features.keys().map(Feature::from).collect();
        let mut has_namespaced_features = false; // features with `dep:` prefix

        // package.features.values() does not provide a way to determine the `dep:` specified by the user.
        for names in manifest.features.values() {
            for name in names {
                if name.starts_with("dep:") {
                    has_namespaced_features = true;
                    break;
                }
            }
        }
        let optional_deps_start = features.len();
        // When namespace dependency is used, other optional dependencies are also not
        // treated as implicit features.
        if !has_namespaced_features {
            for name in package.optional_deps() {
                let feature = Feature::from(name);
                if !features.contains(&feature) {
                    features.push(feature);
                }
            }
        }
        let deps_features_start = features.len();

        if include_deps_features {
            let node = &metadata.resolve.nodes[id];
            // TODO: Unpublished dependencies are not included in `node.deps`.
            for dep in node.deps.iter().filter(|dep| {
                // ignore if `dep_kinds` is empty (i.e., not Rust 1.41+), target specific or not a normal dependency.
                dep.dep_kinds.iter().any(|kind| kind.kind.is_none() && kind.target.is_none())
            }) {
                let dep_package = &metadata.packages[&dep.pkg];
                // TODO: `dep.name` (`resolve.nodes[].deps[].name`) is a valid rust identifier, not a valid feature flag.
                // And `packages[].dependencies` doesn't have package identifier,
                // so I'm not sure if there is a way to find the actual feature name exactly.
                if let Some(d) = package.dependencies.iter().find(|d| d.name == dep_package.name) {
                    let name = d.rename.as_ref().unwrap_or(&d.name);
                    features.extend(dep_package.features.keys().map(|f| Feature::path(name, f)));
                }
                // TODO: Optional deps of `dep_package`.
            }
        }

        Self { features, optional_deps_start, deps_features_start }
    }

    pub(crate) fn normal(&self) -> &[Feature] {
        &self.features[..self.optional_deps_start]
    }

    pub(crate) fn optional_deps(&self) -> &[Feature] {
        &self.features[self.optional_deps_start..self.deps_features_start]
    }

    pub(crate) fn deps_features(&self) -> &[Feature] {
        &self.features[self.deps_features_start..]
    }

    pub(crate) fn contains(&self, name: &str) -> bool {
        self.features.iter().any(|f| f == name)
    }
}

/// The representation of Cargo feature.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Feature {
    /// A feature of the current crate.
    Normal {
        /// Feature name. It is considered indivisible.
        name: String,
    },
    /// Grouped features.
    Group {
        /// Feature name concatenated with `,`.
        name: String,
        /// Original feature list.
        list: Vec<String>,
    },
    /// A feature of a dependency.
    Path {
        /// Feature path separated with `/`.
        name: String,
        /// Index of `/`.
        _slash: usize,
    },
}

impl fmt::Debug for Feature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write;
        match self {
            Self::Normal { name } | Self::Path { name, .. } => f.write_str(name),
            Self::Group { name, .. } => {
                f.write_char('[')?;
                f.write_str(name)?;
                f.write_char(']')
            }
        }
    }
}

impl Feature {
    pub(crate) fn group(group: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let list: Vec<_> = group.into_iter().map(Into::into).collect();
        Self::Group { name: list.join(","), list }
    }

    pub(crate) fn path(parent: &str, name: &str) -> Self {
        Self::Path { name: format!("{parent}/{name}"), _slash: parent.len() }
    }

    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Normal { name } | Self::Group { name, .. } | Self::Path { name, .. } => name,
        }
    }

    pub(crate) fn as_group(&self) -> &[String] {
        match self {
            Self::Group { list, .. } => list,
            Self::Normal { name } | Self::Path { name, .. } => slice::from_ref(name),
        }
    }

    pub(crate) fn matches(&self, s: &str) -> bool {
        self.as_group().iter().any(|n| **n == *s)
    }
}

impl PartialEq<str> for Feature {
    fn eq(&self, other: &str) -> bool {
        self.name() == other
    }
}

impl PartialEq<String> for Feature {
    fn eq(&self, other: &String) -> bool {
        self.name() == other
    }
}

impl<S: Into<String>> From<S> for Feature {
    fn from(name: S) -> Self {
        Self::Normal { name: name.into() }
    }
}

impl AsRef<str> for Feature {
    fn as_ref(&self) -> &str {
        self.name()
    }
}

pub(crate) fn feature_powerset<'a>(
    features: impl IntoIterator<Item = &'a Feature>,
    depth: Option<usize>,
    at_least_one_of: &[Feature],
    mutually_exclusive_features: &[Feature],
    package_features: &BTreeMap<String, Vec<String>>,
) -> Vec<Vec<&'a Feature>> {
    let deps_map = feature_deps(package_features);
    let at_least_one_of = at_least_one_of_for_package(at_least_one_of, &deps_map);

    powerset(features, depth)
        .into_iter()
        .skip(1) // The first element of a powerset is `[]` so it should be skipped.
        .filter(|fs| {
            !fs.iter().any(|f| {
                f.as_group().iter().filter_map(|f| deps_map.get(&&**f)).any(|deps| {
                    fs.iter().any(|f| f.as_group().iter().all(|f| deps.contains(&&**f)))
                })
            })
        })
        .filter(move |fs| {
            // all() returns true if at_least_one_of is empty
            at_least_one_of.iter().all(|required_set| {
                fs
                    .iter()
                    .flat_map(|f| f.as_group())
                    .any(|f| required_set.contains(f.as_str()))
            })
        })
        .filter(move |fs| {
            // Filter any feature set containing more than one feature from the same mutually
            // exclusive group.
            let mut count = 0;
            for f in fs.iter().flat_map(|f| f.as_group()) {
                for group in mutually_exclusive_features {
                    if group.matches(f) {
                        count += 1;
                        if count > 1 {
                            return false;
                        }
                    }
                }
            }
            true
        })
        .collect()
}

fn feature_deps(map: &BTreeMap<String, Vec<String>>) -> BTreeMap<&str, BTreeSet<&str>> {
    fn f<'a>(
        map: &'a BTreeMap<String, Vec<String>>,
        set: &mut BTreeSet<&'a str>,
        cur: &str,
        root: &str,
    ) {
        if let Some(v) = map.get(cur) {
            for x in v {
                // dep: actions aren't features, and can't enable other features in the same crate
                if x.starts_with("dep:") {
                    continue;
                }
                if x != root && set.insert(x) {
                    f(map, set, x, root);
                }
            }
        }
    }
    let mut feat_deps = BTreeMap::new();
    for feat in map.keys() {
        let mut set = BTreeSet::new();
        f(map, &mut set, feat, feat);
        feat_deps.insert(&**feat, set);
    }
    feat_deps
}

fn powerset<T: Copy>(iter: impl IntoIterator<Item = T>, depth: Option<usize>) -> Vec<Vec<T>> {
    iter.into_iter().fold(vec![vec![]], |mut acc, elem| {
        let ext = acc.clone().into_iter().map(|mut cur| {
            cur.push(elem);
            cur
        });
        if let Some(depth) = depth {
            acc.extend(ext.filter(|f| f.len() <= depth));
        } else {
            acc.extend(ext);
        }
        acc
    })
}

// Leave only features that are possible to enable in the package.
pub(crate) fn at_least_one_of_for_package<'a>(
    at_least_one_of: &[Feature],
    package_features_flattened: &BTreeMap<&'a str, BTreeSet<&'a str>>,
) -> Vec<BTreeSet<&'a str>> {
    if at_least_one_of.is_empty() {
        return vec![];
    }

    let mut all_features_enabled_by = BTreeMap::new();
    for (&enabled_by, enables) in package_features_flattened {
        all_features_enabled_by.entry(enabled_by).or_insert_with(BTreeSet::new).insert(enabled_by);
        for &enabled_feature in enables {
            all_features_enabled_by
                .entry(enabled_feature)
                .or_insert_with(BTreeSet::new)
                .insert(enabled_by);
        }
    }

    at_least_one_of
        .iter()
        .map(|set| {
            set.as_group()
                .iter()
                .filter_map(|f| all_features_enabled_by.get(f.as_str()))
                .flat_map(|f| f.iter().copied())
                .collect::<BTreeSet<_>>()
        })
        .filter(|set| !set.is_empty())
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{at_least_one_of_for_package, feature_deps, feature_powerset, powerset, Feature};

    macro_rules! v {
        ($($expr:expr),* $(,)?) => {
            vec![$($expr.into()),*]
        };
    }

    macro_rules! map {
        ($(($key:expr, $value:expr)),* $(,)?) => {
            BTreeMap::from_iter(vec![$(($key.into(), $value)),*])
        };
    }

    macro_rules! set {
        ($($expr:expr),* $(,)?) => {
            BTreeSet::from_iter(vec![$($expr),*])
        };
    }

    #[test]
    fn at_least_one_of_for_package_filter() {
        let map = map![("a", v![]), ("b", v!["a"]), ("c", v!["b"]), ("d", v!["a", "b"])];
        let fd = feature_deps(&map);
        let list: Vec<Feature> = v!["b", "x", "y", "z"];
        let filtered = at_least_one_of_for_package(&list, &fd);
        assert_eq!(filtered, vec![set!("b", "c", "d")]);
    }

    #[test]
    fn powerset_with_filter() {
        let map = map![("a", v![]), ("b", v!["a"]), ("c", v!["b"]), ("d", v!["a", "b"])];

        let list = v!["a", "b", "c", "d"];
        let filtered = feature_powerset(&list, None, &[], &[], &map);
        assert_eq!(filtered, vec![vec!["a"], vec!["b"], vec!["c"], vec!["d"], vec!["c", "d"]]);

        let filtered = feature_powerset(&list, None, &["a".into()], &[], &map);
        assert_eq!(filtered, vec![vec!["a"], vec!["b"], vec!["c"], vec!["d"], vec!["c", "d"]]);

        let filtered = feature_powerset(&list, None, &["c".into()], &[], &map);
        assert_eq!(filtered, vec![vec!["c"], vec!["c", "d"]]);

        let filtered = feature_powerset(&list, None, &["a".into(), "c".into()], &[], &map);
        assert_eq!(filtered, vec![vec!["c"], vec!["c", "d"]]);

        let map = map![("tokio", v![]), ("async-std", v![]), ("a", v![]), ("b", v!["a"])];
        let list = v!["a", "b", "tokio", "async-std"];
        let filtered =
            feature_powerset(&list, None, &[], &["tokio".into(), "async-std".into()], &map);
        assert_eq!(filtered, vec![
            vec!["a"],
            vec!["b"],
            vec!["tokio"],
            vec!["a", "tokio"],
            vec!["b", "tokio"],
            vec!["async-std"],
            vec!["a", "async-std"],
            vec!["b", "async-std"]
        ]);
    }

    #[test]
    fn feature_deps1() {
        let map = map![("a", v![]), ("b", v!["a"]), ("c", v!["b"]), ("d", v!["a", "b"])];
        let fd = feature_deps(&map);
        assert_eq!(fd, map![
            ("a", set![]),
            ("b", set!["a"]),
            ("c", set!["a", "b"]),
            ("d", set!["a", "b"])
        ]);
        let list: Vec<Feature> = v!["a", "b", "c", "d"];
        let ps = powerset(&list, None);
        assert_eq!(ps, vec![
            vec![],
            vec!["a"],
            vec!["b"],
            vec!["a", "b"],
            vec!["c"],
            vec!["a", "c"],
            vec!["b", "c"],
            vec!["a", "b", "c"],
            vec!["d"],
            vec!["a", "d"],
            vec!["b", "d"],
            vec!["a", "b", "d"],
            vec!["c", "d"],
            vec!["a", "c", "d"],
            vec!["b", "c", "d"],
            vec!["a", "b", "c", "d"],
        ]);
        let filtered = feature_powerset(&list, None, &[], &[], &map);
        assert_eq!(filtered, vec![vec!["a"], vec!["b"], vec!["c"], vec!["d"], vec!["c", "d"]]);
    }

    #[test]
    fn powerset_full() {
        let v = powerset(vec![1, 2, 3, 4], None);
        assert_eq!(v, vec![
            vec![],
            vec![1],
            vec![2],
            vec![1, 2],
            vec![3],
            vec![1, 3],
            vec![2, 3],
            vec![1, 2, 3],
            vec![4],
            vec![1, 4],
            vec![2, 4],
            vec![1, 2, 4],
            vec![3, 4],
            vec![1, 3, 4],
            vec![2, 3, 4],
            vec![1, 2, 3, 4],
        ]);
    }

    #[test]
    fn powerset_depth1() {
        let v = powerset(vec![1, 2, 3, 4], Some(1));
        assert_eq!(v, vec![vec![], vec![1], vec![2], vec![3], vec![4],]);
    }

    #[test]
    fn powerset_depth2() {
        let v = powerset(vec![1, 2, 3, 4], Some(2));
        assert_eq!(v, vec![
            vec![],
            vec![1],
            vec![2],
            vec![1, 2],
            vec![3],
            vec![1, 3],
            vec![2, 3],
            vec![4],
            vec![1, 4],
            vec![2, 4],
            vec![3, 4],
        ]);
    }

    #[test]
    fn powerset_depth3() {
        let v = powerset(vec![1, 2, 3, 4], Some(3));
        assert_eq!(v, vec![
            vec![],
            vec![1],
            vec![2],
            vec![1, 2],
            vec![3],
            vec![1, 3],
            vec![2, 3],
            vec![1, 2, 3],
            vec![4],
            vec![1, 4],
            vec![2, 4],
            vec![1, 2, 4],
            vec![3, 4],
            vec![1, 3, 4],
            vec![2, 3, 4],
        ]);
    }
}
