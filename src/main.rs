#![forbid(unsafe_code)]
#![warn(rust_2018_idioms, single_use_lifetimes, unreachable_pub)]
#![warn(clippy::pedantic)]
#![allow(clippy::cast_lossless, clippy::struct_excessive_bools, clippy::too_many_lines)]

#[macro_use]
mod term;

#[macro_use]
mod process;

mod cargo;
mod cli;
mod context;
mod features;
mod fs;
mod manifest;
mod metadata;
mod multithread;
mod restore;
mod rustup;
mod version;

use std::{
    collections::BTreeMap,
    env,
    fmt::{self, Write},
    sync,
};

use anyhow::{bail, Result};
pub const CURRENT: &str = env!("CARGO_MANIFEST_DIR");
use multithread::{unpoison_mutex, TargetDirPool};
use rayon::prelude::*;

use crate::{
    context::Context, features::Feature, metadata::PackageId, process::ProcessBuilder,
    rustup::Rustup,
};

fn main() {
    term::init_coloring();
    if let Err(e) = try_main() {
        error!("{e:#}");
    }
    if term::error()
        || term::warn() && env::var_os("CARGO_HACK_DENY_WARNINGS").filter(|v| v == "true").is_some()
    {
        std::process::exit(1)
    }
}

fn try_main() -> Result<()> {
    let cx = &Context::new()?;

    exec_on_workspace(cx)
}

fn exec_on_workspace(cx: &Context) -> Result<()> {
    let restore_handles = if cx.no_dev_deps || cx.remove_dev_deps {
        let mut restore_handles = Vec::with_capacity(cx.metadata.workspace_members.len());
        for id in &cx.metadata.workspace_members {
            let manifest_path = &cx.packages(id).manifest_path;
            let manifest = cx.manifests(id);
            let doc = manifest.remove_dev_deps();
            restore_handles.push(cx.restore.register(&manifest.raw, manifest_path));
            if term::verbose() {
                info!("removing dev-dependencies from {}", manifest_path.display());
            }
            fs::write(manifest_path, doc)?;
        }
        restore_handles
    } else {
        vec![]
    };

    if cx.subcommand.is_none() {
        // Restore original Cargo.toml and Cargo.lock.
        drop(restore_handles);
        return Ok(());
    }

    let progress = sync::Arc::new(sync::Mutex::new(Progress::default()));
    let packages = determine_package_list(cx, &progress)?;
    let keep_going = sync::Arc::new(sync::Mutex::new(KeepGoing::default()));
    if let Some(range) = &cx.version_range {
        let line = {
            let mut progress = unpoison_mutex(progress.lock());
            let total = progress.total;
            progress.total = 0;
            for (cargo_version, _) in range {
                if cx.target.is_empty() || *cargo_version >= 64 {
                    progress.total += total;
                } else {
                    progress.total += total * cx.target.len();
                }
            }
            cmd!("cargo")
        };

        {
            // First, generate the lockfile using the oldest cargo specified.
            // https://github.com/taiki-e/cargo-hack/issues/105
            let toolchain = &range[0].1;
            rustup::install_toolchain(toolchain, &cx.target, true)?;
            let mut line = line.clone();
            line.leading_arg(toolchain);
            line.arg("generate-lockfile");
            if let Some(pid) = cx.current_package() {
                let package = cx.packages(pid);
                if !cx.no_manifest_path {
                    line.arg("--manifest-path");
                    line.arg(
                        package
                            .manifest_path
                            .strip_prefix(&cx.current_dir)
                            .unwrap_or(&package.manifest_path),
                    );
                }
            }
            line.run_with_output()?;
        }

        range.iter().enumerate().try_for_each(|(i, (cargo_version, toolchain))| {
            if i != 0 {
                rustup::install_toolchain(toolchain, &cx.target, true)?;
            }

            if cx.clean_per_version {
                cargo_clean(cx, None)?;
            }

            let mut line = line.clone();
            line.leading_arg(toolchain);
            line.apply_context(cx);
            exec_on_packages(cx, &packages, line, &progress, &keep_going, *cargo_version)
        })?;
    } else {
        let mut line = cx.cargo();
        line.apply_context(cx);
        exec_on_packages(cx, &packages, line, &progress, &keep_going, cx.cargo_version)?;
    }
    let keep_going = unpoison_mutex(keep_going.lock());
    if keep_going.count > 0 {
        eprintln!();
        error!("{keep_going}");
    }

    // Restore original Cargo.toml and Cargo.lock.
    drop(restore_handles);

    Ok(())
}

#[derive(Default)]
struct Progress {
    total: usize,
    count: usize,
}

enum Kind<'a> {
    SkipAsPrivate,
    Normal,
    Each { features: Vec<&'a Feature> },
    Powerset { features: Vec<Vec<&'a Feature>> },
}

impl ToString for Kind<'_> {
    fn to_string(&self) -> String {
        String::from(match *self {
            Kind::SkipAsPrivate => "skip_as_private",
            Kind::Normal => "normal",
            Kind::Each { .. } => "each",
            Kind::Powerset { .. } => "powerest",
        })
    }
}

fn determine_kind<'a>(
    cx: &'a Context,
    id: &PackageId,
    progress: &sync::Arc<sync::Mutex<Progress>>,
    multiple_packages: bool,
) -> Kind<'a> {
    let mut progress = unpoison_mutex(progress.lock());
    assert!(cx.subcommand.is_some());
    if cx.ignore_private && cx.is_private(id) {
        info!("skipped running on private package `{}`", cx.name_verbose(id));
        return Kind::SkipAsPrivate;
    }
    if !cx.each_feature && !cx.feature_powerset {
        progress.total += 1;
        return Kind::Normal;
    }

    let package = cx.packages(id);
    let pkg_features = cx.pkg_features(id);
    let filter = |&f: &&Feature| {
        !cx.exclude_features.iter().any(|s| f == s)
            && !cx.group_features.iter().any(|g| g.matches(f.name()))
    };
    let features = if cx.include_features.is_empty() {
        // TODO
        if !multiple_packages {
            for name in &cx.exclude_features {
                if !pkg_features.contains(name) {
                    warn!("specified feature `{name}` not found in package `{}`", package.name);
                }
            }
        }

        let mut features: Vec<_> = pkg_features.normal().iter().filter(filter).collect();

        if let Some(opt_deps) = &cx.optional_deps {
            if opt_deps.len() == 1 && opt_deps[0].is_empty() {
                // --optional-deps=
            } else if !multiple_packages {
                // TODO
                for d in opt_deps {
                    if !pkg_features.optional_deps().iter().any(|f| f == d) {
                        warn!(
                            "specified optional dependency `{d}` not found in package `{}`",
                            package.name
                        );
                    }
                }
            }

            features.extend(pkg_features.optional_deps().iter().filter(|f| {
                filter(f) && (opt_deps.is_empty() || opt_deps.iter().any(|x| *f == x))
            }));
        }

        if cx.include_deps_features {
            features.extend(pkg_features.deps_features().iter().filter(filter));
        }

        if !cx.group_features.is_empty() {
            features.extend(cx.group_features.iter());
        }

        features
    } else {
        cx.include_features.iter().filter(filter).collect()
    };

    if cx.each_feature {
        if (pkg_features.normal().is_empty() && pkg_features.optional_deps().is_empty()
            || !cx.include_features.is_empty())
            && features.is_empty()
        {
            progress.total += 1;
            Kind::Normal
        } else {
            progress.total += features.len()
                + !cx.exclude_no_default_features as usize
                + (!cx.exclude_all_features
                    && pkg_features.normal().len() + pkg_features.optional_deps().len() > 1)
                    as usize;
            Kind::Each { features }
        }
    } else if cx.feature_powerset {
        let features = features::feature_powerset(features, cx.depth, &package.features);

        if (pkg_features.normal().is_empty() && pkg_features.optional_deps().is_empty()
            || !cx.include_features.is_empty())
            && features.is_empty()
        {
            progress.total += 1;
            Kind::Normal
        } else {
            // -1: the first element of a powerset is `[]`
            progress.total += features.len() - 1
                + !cx.exclude_no_default_features as usize
                + (!cx.exclude_all_features
                    && pkg_features.normal().len() + pkg_features.optional_deps().len() > 1)
                    as usize;
            Kind::Powerset { features }
        }
    } else {
        unreachable!()
    }
}

fn determine_package_list<'a>(
    cx: &'a Context,
    progress: &sync::Arc<sync::Mutex<Progress>>,
) -> Result<Vec<(&'a PackageId, Kind<'a>)>> {
    Ok(if cx.workspace {
        for spec in &cx.exclude {
            if !cx.workspace_members().any(|id| cx.packages(id).name == *spec) {
                warn!(
                    "excluded package(s) `{spec}` not found in workspace `{}`",
                    cx.workspace_root().display()
                );
            }
        }

        let multiple_packages = cx.workspace_members().len().saturating_sub(cx.exclude.len()) > 1;
        cx.workspace_members()
            .filter(|id| !cx.exclude.contains(&cx.packages(id).name))
            .map(|id| (id, determine_kind(cx, id, progress, multiple_packages)))
            .collect()
    } else if !cx.package.is_empty() {
        if let Some(spec) = cx
            .package
            .iter()
            .find(|&spec| !cx.workspace_members().any(|id| cx.packages(id).name == *spec))
        {
            bail!("package ID specification `{spec}` matched no packages")
        }

        let multiple_packages = cx.package.len() > 1;
        cx.workspace_members()
            .filter(|id| cx.package.contains(&cx.packages(id).name))
            .map(|id| (id, determine_kind(cx, id, progress, multiple_packages)))
            .collect()
    } else if cx.current_package().is_none() {
        let multiple_packages = cx.workspace_members().len() > 1;
        cx.workspace_members()
            .map(|id| (id, determine_kind(cx, id, progress, multiple_packages)))
            .collect()
    } else {
        let current_package = &cx.packages(cx.current_package().unwrap()).name;
        let multiple_packages = false;
        cx.workspace_members()
            .find(|id| cx.packages(id).name == *current_package)
            .map(|id| vec![(id, determine_kind(cx, id, progress, multiple_packages))])
            .unwrap_or_default()
    })
}

fn exec_on_packages(
    cx: &Context,
    packages: &[(&PackageId, Kind<'_>)],
    mut line: ProcessBuilder<'_>,
    progress: &sync::Arc<sync::Mutex<Progress>>,
    keep_going: &sync::Arc<sync::Mutex<KeepGoing>>,
    cargo_version: u32,
) -> Result<()> {
    let target_dirs = TargetDirPool::new();
    if cx.target.is_empty() || cargo_version >= 64 {
        // TODO: Test that cargo multitarget does not break the resolver behavior required for a correct check.
        for target in &cx.target {
            line.arg("--target");
            line.arg(target);
        }
        packages.par_iter().try_for_each(|(id, kind)| {
            exec_on_package(cx, id, kind, &line, progress.clone(), keep_going.clone(), &target_dirs)
        })
    } else {
        cx.target.iter().try_for_each(|target| {
            let mut line = line.clone();
            line.arg("--target");
            line.arg(target);
            packages.par_iter().try_for_each(|(id, kind)| {
                exec_on_package(
                    cx,
                    id,
                    kind,
                    &line,
                    progress.clone(),
                    keep_going.clone(),
                    &target_dirs,
                )
            })
        })
    }
}

#[allow(clippy::needless_pass_by_value)]
fn exec_on_package(
    cx: &Context,
    id: &PackageId,
    kind: &Kind<'_>,
    line: &ProcessBuilder<'_>,
    progress: sync::Arc<sync::Mutex<Progress>>,
    keep_going: sync::Arc<sync::Mutex<KeepGoing>>,
    target_dirs: &TargetDirPool,
) -> Result<()> {
    if let Kind::SkipAsPrivate = kind {
        return Ok(());
    }

    let package = cx.packages(id);

    let mut line = line.clone();
    line.append_features_from_args(cx, id);

    if !cx.no_manifest_path {
        line.arg("--manifest-path");
        line.arg(
            package.manifest_path.strip_prefix(&cx.current_dir).unwrap_or(&package.manifest_path),
        );
    }

    exec_actual(cx, id, kind, &mut line, &progress, &keep_going, target_dirs)
}

fn exec_actual(
    cx: &Context,
    id: &PackageId,
    kind: &Kind<'_>,
    line: &mut ProcessBuilder<'_>,
    progress: &sync::Arc<sync::Mutex<Progress>>,
    keep_going: &sync::Arc<sync::Mutex<KeepGoing>>,
    target_dirs: &TargetDirPool,
) -> Result<()> {
    match kind {
        Kind::SkipAsPrivate => unreachable!(),
        Kind::Normal => {
            // only run with default features
            return exec_cargo(cx, id, line, progress, keep_going, target_dirs);
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
        exec_cargo(cx, id, &mut line, progress, keep_going, target_dirs)?;
    }

    match kind {
        Kind::Each { features } => {
            features.iter().try_for_each(|f| {
                exec_cargo_with_features(cx, id, &line, progress, keep_going, Some(f), target_dirs)
            })?;
        }
        Kind::Powerset { features } => {
            // The first element of a powerset is `[]` so it should be skipped.
            features.iter().skip(1).try_for_each(|f| {
                exec_cargo_with_features(cx, id, &line, progress, keep_going, f, target_dirs)
            })?;
        }
        _ => unreachable!(),
    }

    let pkg_features = cx.pkg_features(id);
    if !cx.exclude_all_features
        && pkg_features.normal().len() + pkg_features.optional_deps().len() > 1
    {
        // run with all features
        // https://github.com/taiki-e/cargo-hack/issues/42
        line.arg("--all-features");
        exec_cargo(cx, id, &mut line, progress, keep_going, target_dirs)?;
    }

    Ok(())
}

fn exec_cargo_with_features(
    cx: &Context,
    id: &PackageId,
    line: &ProcessBuilder<'_>,
    progress: &sync::Arc<sync::Mutex<Progress>>,
    keep_going: &sync::Arc<sync::Mutex<KeepGoing>>,
    features: impl IntoIterator<Item = impl AsRef<str>>,
    target_dirs: &TargetDirPool,
) -> Result<()> {
    let mut line = line.clone();
    line.append_features(features);
    exec_cargo(cx, id, &mut line, progress, keep_going, target_dirs)
}

#[derive(Default)]
struct KeepGoing {
    count: u64,
    failed_commands: BTreeMap<String, Vec<String>>,
}

impl fmt::Display for KeepGoing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "failed to run {} commands", self.count)?;
        writeln!(f)?;
        writeln!(f, "failed commands:")?;
        for (pkg, commands) in &self.failed_commands {
            writeln!(f, "    {pkg}:")?;
            for cmd in commands {
                writeln!(f, "        {cmd}")?;
            }
        }
        Ok(())
    }
}

fn exec_cargo(
    cx: &Context,
    id: &PackageId,
    line: &mut ProcessBuilder<'_>,
    progress: &sync::Arc<sync::Mutex<Progress>>,
    keep_going: &sync::Arc<sync::Mutex<KeepGoing>>,
    target_dirs: &TargetDirPool,
) -> Result<()> {
    let res = exec_cargo_inner(cx, id, line, progress, target_dirs);
    if cx.keep_going {
        if let Err(e) = res {
            let mut keep_going = unpoison_mutex(keep_going.lock());
            error!("{e:#}");
            keep_going.count = keep_going.count.saturating_add(1);
            let name = cx.packages(id).name.clone();
            if !keep_going.failed_commands.contains_key(&name) {
                keep_going.failed_commands.insert(name.clone(), vec![]);
            }
            keep_going.failed_commands.get_mut(&name).unwrap().push(format!("{line:#}"));
        }
        Ok(())
    } else {
        res
    }
}

fn exec_cargo_inner(
    cx: &Context,
    id: &PackageId,
    line: &mut ProcessBuilder<'_>,
    progress: &sync::Arc<sync::Mutex<Progress>>,
    target_dirs: &TargetDirPool,
) -> Result<()> {
    {
        let mut progress = unpoison_mutex(progress.lock());
        if progress.count != 0 {
            eprintln!();
        }
        progress.count += 1;

        if cx.clean_per_run {
            cargo_clean(cx, Some(id))?;
        }

        // running `<command>` (on <package>) (<count>/<total>)
        let mut msg = String::new();
        if term::verbose() {
            write!(msg, "running {line}").unwrap();
        } else {
            write!(msg, "running {line} on {}", cx.packages(id).name).unwrap();
        }
        write!(msg, " ({}/{})", progress.count, progress.total).unwrap();
        info!("{msg}");
    }

    let target_dir_inner = target_dirs.get();
    let target_dir = env::current_dir().unwrap().join("target").join(&target_dir_inner);
    // line.arg("--target-dir");
    // line.arg(target_dir);
    line.run_with_env(("CARGO_TARGET_DIR", target_dir.to_str().unwrap()))?;
    target_dirs.give_back(target_dir_inner);
    Ok(())
}

fn cargo_clean(cx: &Context, id: Option<&PackageId>) -> Result<()> {
    let mut line = cx.cargo();
    line.arg("clean");
    if let Some(id) = id {
        line.arg("--package");
        line.arg(&cx.packages(id).name);
    }

    if term::verbose() {
        // running `cargo clean [--package <package>]`
        info!("running {line}");
    }

    line.run()
}
