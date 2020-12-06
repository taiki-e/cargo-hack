#![forbid(unsafe_code)]
#![warn(future_incompatible, rust_2018_idioms, single_use_lifetimes, unreachable_pub)]
#![warn(clippy::all, clippy::default_trait_access)]
// mem::take, #[non_exhaustive], and Option::{as_deref, as_deref_mut} require Rust 1.40,
// matches! requires Rust 1.42, str::{strip_prefix, strip_suffix} requires Rust 1.45
#![allow(
    clippy::mem_replace_with_default,
    clippy::manual_non_exhaustive,
    clippy::option_as_ref_deref,
    clippy::match_like_matches_macro,
    clippy::manual_strip
)]

#[macro_use]
mod term;

mod cargo;
mod cli;
mod context;
mod features;
mod manifest;
mod metadata;
mod process;
mod remove_dev_deps;
mod restore;
mod rustup;
mod version;

use anyhow::{bail, Context as _};
use std::{fmt::Write, fs};

use crate::{
    cargo::Cargo, context::Context, features::Feature, metadata::PackageId,
    process::ProcessBuilder, restore::Restore, rustup::Rustup,
};

type Result<T, E = anyhow::Error> = std::result::Result<T, E>;

fn main() {
    if let Err(e) = try_main() {
        error!("{:#}", e);
        std::process::exit(1)
    }
}

fn try_main() -> Result<()> {
    let args = &cli::raw();
    let cx = &Context::new(args)?;

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

    let mut progress = Progress::default();
    let packages = determine_package_list(cx, &mut progress)?;
    let restore = Restore::new(cx);
    if let Some(range) = &cx.version_range {
        progress.total *= range.len();
        let mut line = ProcessBuilder::new("cargo".as_ref());
        if cx.verbose {
            line.display_manifest_path();
        }
        {
            // First, generate the lockfile using the oldest cargo specified.
            // https://github.com/taiki-e/cargo-hack/issues/105
            let toolchain = &range[0];
            rustup::install_toolchain(toolchain)?;
            let mut line = line.clone();
            line.leading_arg(toolchain);
            line.args(&["generate-lockfile"]);
            if let Some(pid) = cx.current_package() {
                let package = cx.packages(pid);
                line.arg("--manifest-path");
                line.arg(
                    package
                        .manifest_path
                        .strip_prefix(&cx.current_dir)
                        .unwrap_or(&package.manifest_path),
                );
            }
            line.exec_with_output()?;
        }

        range.iter().try_for_each(|toolchain| {
            rustup::install_toolchain(toolchain)?;
            let mut line = line.clone();
            line.leading_arg(toolchain);
            line.with_args(cx);
            packages.iter().try_for_each(|(id, kind)| {
                exec_on_package(cx, id, kind, &line, &restore, &mut progress)
            })
        })
    } else {
        let mut line = cx.cargo();
        line.with_args(cx);
        packages.iter().try_for_each(|(id, kind)| {
            exec_on_package(cx, id, kind, &line, &restore, &mut progress)
        })
    }
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
    Each { features: Vec<&'a Feature> },
    Powerset { features: Vec<Vec<&'a Feature>> },
}

fn determine_kind<'a>(cx: &'a Context<'_>, id: &PackageId, progress: &mut Progress) -> Kind<'a> {
    if cx.ignore_private && cx.is_private(id) {
        info!("skipped running on private package `{}`", cx.name_verbose(id));
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
    let filter = |&f: &&Feature| {
        !cx.exclude_features.iter().any(|s| f == *s)
            && !cx.group_features.iter().any(|g| g.matches(f.as_ref()))
    };
    let features = if cx.include_features.is_empty() {
        let feature_list = cx.pkg_features(id);

        cx.exclude_features.iter().for_each(|d| {
            if !feature_list.contains(d) {
                warn!("specified feature `{}` not found in package `{}`", d, package.name);
            }
        });

        let mut features: Vec<_> = feature_list.normal().iter().filter(filter).collect();

        if let Some(opt_deps) = &cx.optional_deps {
            opt_deps.iter().for_each(|&d| {
                if !feature_list.optional_deps().iter().any(|f| f == d) {
                    warn!(
                        "specified optional dependency `{}` not found in package `{}`",
                        d, package.name
                    );
                }
            });

            features.extend(feature_list.optional_deps().iter().filter(|f| {
                filter(f) && (opt_deps.is_empty() || opt_deps.iter().any(|x| *f == *x))
            }));
        }

        if cx.include_deps_features {
            features.extend(feature_list.deps_features().iter().filter(filter));
        }

        if !cx.group_features.is_empty() {
            features.extend(cx.group_features.iter());
        }

        features
    } else {
        cx.include_features.iter().filter(filter).collect()
    };

    if cx.each_feature {
        if (package.features.is_empty() || !cx.include_features.is_empty()) && features.is_empty() {
            progress.total += 1;
            Kind::Nomal
        } else {
            progress.total += features.len()
                + !cx.exclude_no_default_features as usize
                + !cx.exclude_all_features as usize;
            Kind::Each { features }
        }
    } else if cx.feature_powerset {
        let features = features::feature_powerset(features, cx.depth, &package.features);

        if (package.features.is_empty() || !cx.include_features.is_empty()) && features.is_empty() {
            progress.total += 1;
            Kind::Nomal
        } else {
            // -1: the first element of a powerset is `[]`
            progress.total += features.len() - 1
                + !cx.exclude_no_default_features as usize
                + !cx.exclude_all_features as usize;
            Kind::Powerset { features }
        }
    } else {
        unreachable!()
    }
}

// https://github.com/rust-lang/rust-clippy/issues/6384
#[allow(clippy::unnecessary_wraps)]
fn determine_package_list<'a>(
    cx: &'a Context<'_>,
    progress: &mut Progress,
) -> Result<Vec<(&'a PackageId, Kind<'a>)>> {
    Ok(if cx.workspace {
        cx.exclude.iter().for_each(|spec| {
            if !cx.workspace_members().any(|id| cx.packages(id).name == *spec) {
                warn!(
                    "excluded package(s) `{}` not found in workspace `{}`",
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
    if let Kind::SkipAsPrivate = kind {
        return Ok(());
    }

    let package = cx.packages(id);

    let mut line = line.clone();
    line.append_features_from_args(cx, id);

    line.arg("--manifest-path");
    line.arg(package.manifest_path.strip_prefix(&cx.current_dir).unwrap_or(&package.manifest_path));

    if cx.no_dev_deps || cx.remove_dev_deps {
        let new = cx.manifests(id).remove_dev_deps();
        let mut handle = restore.set_manifest(cx, id);

        fs::write(&package.manifest_path, new).with_context(|| {
            format!("failed to update manifest file `{}`", package.manifest_path.display())
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

    if !cx.no_default_features {
        line.arg("--no-default-features");
    }

    // if `metadata.packages[].features` has `default` feature, users can
    // specify `--features=default`, so it should be one of the combinations.
    // Otherwise, "run with default features" is basically the same as
    // "run with no default features".

    if !cx.exclude_no_default_features {
        // run with no default features if the package has other features
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
    info!("{}", msg);

    line.exec()
}

fn cargo_clean(cx: &Context<'_>, id: &PackageId) -> Result<()> {
    let mut line = cx.cargo();
    line.arg("clean");
    line.arg("--package");
    line.arg(&cx.packages(id).name);

    if cx.verbose {
        // running `cargo clean --package <package>`
        info!("running {}", line);
    }

    line.exec()
}
