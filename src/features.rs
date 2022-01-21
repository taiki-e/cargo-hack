use std::{
    collections::{BTreeMap, BTreeSet},
    slice,
};

use crate::{metadata::Metadata, PackageId};

#[derive(Debug)]
pub(crate) struct Features {
    features: Vec<Feature>,
    optional_deps_start: usize,
    deps_features_start: usize,
}

impl Features {
    pub(crate) fn new(metadata: &Metadata, id: &PackageId) -> Self {
        let package = &metadata.packages[id];
        let node = &metadata.resolve.nodes[id];

        let mut features = Vec::with_capacity(package.features.len());
        let mut optional_deps = vec![];

        for name in package.optional_deps() {
            optional_deps.push(name);
        }
        for name in package.features.keys() {
            if !optional_deps.contains(&&**name) {
                features.push(name.into());
            }
        }
        let optional_deps_start = features.len();
        features.extend(optional_deps.into_iter().map(Into::into));
        let deps_features_start = features.len();

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
#[derive(Debug)]
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

impl Feature {
    pub(crate) fn group(group: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let list: Vec<_> = group.into_iter().map(Into::into).collect();
        Self::Group { name: list.join(","), list }
    }

    pub(crate) fn path(parent: &str, name: &str) -> Self {
        Self::Path { name: format!("{}/{}", parent, name), _slash: parent.len() }
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
    map: &BTreeMap<String, Vec<String>>,
) -> Vec<Vec<&'a Feature>> {
    let deps_map = feature_deps(map);
    powerset(features, depth)
        .into_iter()
        .filter(|fs| {
            !fs.iter().any(|f| {
                f.as_group().iter().filter_map(|f| deps_map.get(&&**f)).any(|deps| {
                    fs.iter().any(|f| f.as_group().iter().all(|f| deps.contains(&&**f)))
                })
            })
        })
        .collect()
}

fn feature_deps(map: &BTreeMap<String, Vec<String>>) -> BTreeMap<&str, BTreeSet<&str>> {
    let mut feat_deps = BTreeMap::new();
    for feat in map.keys() {
        let mut set = BTreeSet::new();
        fn f<'a>(
            map: &'a BTreeMap<String, Vec<String>>,
            set: &mut BTreeSet<&'a str>,
            curr: &str,
            root: &str,
        ) {
            if let Some(v) = map.get(curr) {
                for x in v {
                    if x != root && set.insert(x) {
                        f(map, set, x, root);
                    }
                }
            }
        }
        f(map, &mut set, feat, feat);
        feat_deps.insert(&**feat, set);
    }
    feat_deps
}

fn powerset<T: Copy>(iter: impl IntoIterator<Item = T>, depth: Option<usize>) -> Vec<Vec<T>> {
    iter.into_iter().fold(vec![vec![]], |mut acc, elem| {
        let ext = acc.clone().into_iter().map(|mut curr| {
            curr.push(elem);
            curr
        });
        if let Some(depth) = depth {
            acc.extend(ext.filter(|f| f.len() <= depth));
        } else {
            acc.extend(ext);
        }
        acc
    })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        iter::FromIterator,
    };

    use super::{feature_deps, feature_powerset, powerset, Feature};

    macro_rules! svec {
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
    fn feature_deps1() {
        let map =
            map![("a", svec![]), ("b", svec!["a"]), ("c", svec!["b"]), ("d", svec!["a", "b"])];
        let fd = feature_deps(&map);
        assert_eq!(fd, map![
            ("a", set![]),
            ("b", set!["a"]),
            ("c", set!["a", "b"]),
            ("d", set!["a", "b"])
        ]);
        let list: Vec<Feature> = svec!["a", "b", "c", "d"];
        let ps = powerset(list.iter().collect::<Vec<_>>(), None);
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
        let filtered = feature_powerset(list.iter().collect::<Vec<_>>(), None, &map);
        assert_eq!(filtered, vec![vec![], vec!["a"], vec!["b"], vec!["c"], vec!["d"], vec![
            "c", "d"
        ]]);
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
