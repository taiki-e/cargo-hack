use crate::{
    metadata::{Dependency, Metadata},
    PackageId,
};

pub(crate) struct Features {
    features: Vec<String>,
    /// [package features len, package features + optional deps len]
    len: [usize; 2],
}

impl Features {
    pub(crate) fn new(metadata: &Metadata, id: &PackageId) -> Self {
        let package = &metadata.packages[id];
        let node = &metadata.resolve.nodes[id];

        let mut features = Vec::with_capacity(package.features.len());

        for name in package.features.keys().cloned() {
            features.push(name);
        }
        for name in package.dependencies.iter().filter_map(Dependency::as_feature) {
            features.push(name.to_string());
        }
        let len = [package.features.len(), features.len()];

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
                features.extend(dep_package.features.keys().map(|f| format!("{}/{}", name, f)));
            }
            // TODO: Optional deps of `dep_package`.
        }

        Self { features, len }
    }

    pub(crate) fn normal(&self) -> &[String] {
        &self.features[..self.len[0]]
    }

    pub(crate) fn optional_deps(&self) -> &[String] {
        &self.features[self.len[0]..self.len[1]]
    }

    pub(crate) fn deps_features(&self) -> &[String] {
        &self.features[self.len[1]..]
    }

    pub(crate) fn contains(&self, name: &str) -> bool {
        self.features.iter().any(|f| f == name)
    }
}

pub(crate) fn powerset<T: Clone>(
    iter: impl IntoIterator<Item = T>,
    depth: Option<usize>,
) -> Vec<Vec<T>> {
    iter.into_iter().fold(vec![vec![]], |mut acc, elem| {
        let ext = acc.clone().into_iter().map(|mut curr| {
            curr.push(elem.clone());
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
    use super::powerset;

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
