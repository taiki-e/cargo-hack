#![forbid(unsafe_code)]
#![warn(future_incompatible, rust_2018_idioms, single_use_lifetimes, unreachable_pub)]
#![warn(clippy::all, clippy::default_trait_access)]
// mem::take and #[non_exhaustive] requires Rust 1.40, matches! requires Rust 1.42
#![allow(
    clippy::mem_replace_with_default,
    clippy::manual_non_exhaustive,
    clippy::match_like_matches_macro
)]

#[macro_use]
mod term;

mod cli;
mod context;
mod manifest;
mod metadata;
mod process;
mod remove_dev_deps;
mod restore;
mod version;

use anyhow::{bail, Context as _};
use std::{fmt::Write, fs};

use crate::{
    cli::Coloring,
    context::Context,
    metadata::{Dependency, PackageId},
    process::ProcessBuilder,
    restore::Restore,
};

type Result<T, E = anyhow::Error> = std::result::Result<T, E>;

fn main() {
    let mut coloring = None;
    if let Err(e) = try_main(&mut coloring) {
        error!(coloring, "{:#}", e);
        std::process::exit(1)
    }
}

fn try_main(coloring: &mut Option<Coloring>) -> Result<()> {
    let args = &cli::raw();
    let cx = &Context::new(args, coloring)?;

    exec_on_workspace(cx)
}

fn exec_on_workspace(cx: &Context<'_>) -> Result<()> {
    // TODO: Ideally, we should do this, but for now, we allow it as cargo-hack
    // may mistakenly interpret the specified valid feature flag as unknown.
    // if cx.ignore_unknown_features && !cx.workspace && !cx.current_manifest().is_virtual() {
    //     bail!(
    //         "--ignore-unknown-features can only be used in the root of a virtual workspace or together with --workspace"
    //     )
    // }

    let mut line = cx.process().with_args(cx);

    if let Some(color) = cx.color {
        line.arg("--color");
        line.arg(color.as_str());
    }

    let restore = Restore::new(cx);
    let mut progress = Progress::default();
    determine_package_list(cx, &mut progress)?
        .iter()
        .try_for_each(|(id, kind)| exec_on_package(cx, id, kind, &line, &restore, &mut progress))
}

#[derive(Default)]
struct Progress {
    total: usize,
    count: usize,
}

enum Kind<'a> {
    // If there is no subcommand, then kind need not be determined.
    NoSubcommand,
    SkipAsPrivate,
    Nomal,
    Each { features: Vec<&'a str> },
    Powerset { features: Vec<Vec<&'a str>> },
}

fn determine_kind<'a>(cx: &'a Context<'_>, id: &PackageId, progress: &mut Progress) -> Kind<'a> {
    if cx.ignore_private && cx.is_private(id) {
        return Kind::SkipAsPrivate;
    }
    if cx.subcommand.is_none() {
        return Kind::NoSubcommand;
    }
    if !cx.each_feature && !cx.feature_powerset {
        progress.total += 1;
        return Kind::Nomal;
    }

    let package = cx.packages(id);
    let features = if cx.include_features.is_empty() {
        let mut features: Vec<_> = package
            .features
            .iter()
            .map(String::as_str)
            .filter(|f| *f != "default" && !cx.exclude_features.contains(f))
            .collect();
        if let Some(opt_deps) = &cx.optional_deps {
            features.extend(package.dependencies.iter().filter_map(Dependency::as_feature).filter(
                move |&f| {
                    !cx.exclude_features.contains(&f)
                        && (opt_deps.is_empty() || opt_deps.contains(&f))
                },
            ));
        }
        features
    } else {
        cx.include_features
            .iter()
            .filter(|&&f| f != "default" && !cx.exclude_features.contains(&f))
            .copied()
            .collect()
    };

    if cx.each_feature {
        if (package.features.is_empty() || !cx.include_features.is_empty()) && features.is_empty() {
            progress.total += 1;
            Kind::Nomal
        } else {
            progress.total += features.len()
                + (!cx.exclude_features.contains(&"default")) as usize
                + (!cx.exclude_no_default_features) as usize
                + (!cx.exclude_all_features) as usize;
            Kind::Each { features }
        }
    } else if cx.feature_powerset {
        let features = powerset(features, cx.depth);

        if (package.features.is_empty() || !cx.include_features.is_empty()) && features.is_empty() {
            progress.total += 1;
            Kind::Nomal
        } else {
            // -1: the first element of a powerset is `[]`
            progress.total += features.len() - 1
                + (!cx.exclude_features.contains(&"default")) as usize
                + (!cx.exclude_no_default_features) as usize
                + (!cx.exclude_all_features) as usize;
            Kind::Powerset { features }
        }
    } else {
        unreachable!()
    }
}

fn determine_package_list<'a>(
    cx: &'a Context<'_>,
    progress: &mut Progress,
) -> Result<Vec<(&'a PackageId, Kind<'a>)>> {
    Ok(if cx.workspace {
        cx.exclude.iter().for_each(|spec| {
            if !cx.workspace_members().any(|id| cx.packages(id).name == *spec) {
                warn!(
                    cx.color,
                    "excluded package(s) {} not found in workspace `{}`",
                    spec,
                    cx.workspace_root().display()
                );
            }
        });

        cx.workspace_members()
            .filter(|id| !cx.exclude.contains(&&*cx.packages(id).name))
            .map(|id| (id, determine_kind(cx, id, progress)))
            .collect()
    } else if !cx.package.is_empty() {
        if let Some(spec) = cx
            .package
            .iter()
            .find(|&&spec| !cx.workspace_members().any(|id| cx.packages(id).name == spec))
        {
            bail!("package ID specification `{}` matched no packages", spec)
        }

        cx.workspace_members()
            .filter(|id| cx.package.contains(&&*cx.packages(id).name))
            .map(|id| (id, determine_kind(cx, id, progress)))
            .collect()
    } else if cx.current_package().is_none() {
        cx.workspace_members().map(|id| (id, determine_kind(cx, id, progress))).collect()
    } else {
        let current_package = &cx.packages(cx.current_package().unwrap()).name;
        cx.workspace_members()
            .find(|id| cx.packages(id).name == *current_package)
            .map(|id| vec![(id, determine_kind(cx, id, progress))])
            .unwrap_or_default()
    })
}

fn exec_on_package(
    cx: &Context<'_>,
    id: &PackageId,
    kind: &Kind<'_>,
    line: &ProcessBuilder<'_>,
    restore: &Restore,
    progress: &mut Progress,
) -> Result<()> {
    let package = cx.packages(id);
    if let Kind::SkipAsPrivate = kind {
        info!(cx.color, "skipped running on private crate {}", package.name_verbose(cx));
        return Ok(());
    }

    let mut line = line.clone();
    line.append_features_from_args(cx, id);

    line.arg("--manifest-path");
    line.arg(&package.manifest_path);

    if cx.no_dev_deps || cx.remove_dev_deps {
        let new = cx.manifests(id).remove_dev_deps();
        let handle = restore.set_manifest(cx, id);

        fs::write(&package.manifest_path, new).with_context(|| {
            format!("failed to update manifest file: {}", package.manifest_path.display())
        })?;

        exec_actual(cx, id, kind, &mut line, progress)?;

        handle.close()
    } else {
        exec_actual(cx, id, kind, &mut line, progress)
    }
}

fn exec_actual(
    cx: &Context<'_>,
    id: &PackageId,
    kind: &Kind<'_>,
    line: &mut ProcessBuilder<'_>,
    progress: &mut Progress,
) -> Result<()> {
    match kind {
        Kind::NoSubcommand => return Ok(()),
        Kind::SkipAsPrivate => unreachable!(),
        Kind::Nomal => {
            // only run with default features
            return exec_cargo(cx, id, line, progress);
        }
        Kind::Each { .. } | Kind::Powerset { .. } => {}
    }

    let mut line = line.clone();

    if !cx.exclude_features.contains(&"default") {
        // run with default features
        exec_cargo(cx, id, &mut line, progress)?;
    }

    if !cx.no_default_features {
        line.arg("--no-default-features");
    }

    if !cx.exclude_no_default_features {
        // run with no default features if the package has other features
        //
        // `default` is not skipped because `cfg(feature = "default")` is work
        // if `default` feature specified.
        exec_cargo(cx, id, &mut line, progress)?;
    }

    match kind {
        Kind::Each { features } => {
            features
                .iter()
                .try_for_each(|f| exec_cargo_with_features(cx, id, &line, progress, Some(f)))?;
        }
        Kind::Powerset { features } => {
            // The first element of a powerset is `[]` so it should be skipped.
            features
                .iter()
                .skip(1)
                .try_for_each(|f| exec_cargo_with_features(cx, id, &line, progress, f))?;
        }
        _ => unreachable!(),
    }

    if !cx.exclude_all_features {
        // run with all features
        line.arg("--all-features");
        exec_cargo(cx, id, &mut line, progress)?;
    }

    Ok(())
}

fn exec_cargo_with_features(
    cx: &Context<'_>,
    id: &PackageId,
    line: &ProcessBuilder<'_>,
    progress: &mut Progress,
    features: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<()> {
    let mut line = line.clone();
    line.append_features(features);
    exec_cargo(cx, id, &mut line, progress)
}

fn exec_cargo(
    cx: &Context<'_>,
    id: &PackageId,
    line: &mut ProcessBuilder<'_>,
    progress: &mut Progress,
) -> Result<()> {
    progress.count += 1;

    if cx.clean_per_run {
        cargo_clean(cx, id)?;
    }

    // running `<command>` (on <package>) (<count>/<total>)
    let mut msg = String::new();
    if cx.verbose {
        write!(msg, "running {}", line).unwrap();
    } else {
        write!(msg, "running {} on {}", line, cx.packages(id).name).unwrap();
    }
    write!(msg, " ({}/{})", progress.count, progress.total).unwrap();
    info!(cx.color, "{}", msg);

    line.exec()
}

fn cargo_clean(cx: &Context<'_>, id: &PackageId) -> Result<()> {
    let mut line = cx.process();
    line.arg("clean");
    line.arg("--package");
    line.arg(&cx.packages(id).name);

    if cx.verbose {
        // running `cargo clean --package <package>`
        info!(cx.color, "running {}", line);
    }

    line.exec()
}

fn powerset<T: Clone>(iter: impl IntoIterator<Item = T>, depth: Option<usize>) -> Vec<Vec<T>> {
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
