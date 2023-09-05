#![forbid(unsafe_code)]
#![warn(rust_2018_idioms, single_use_lifetimes, unreachable_pub)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_lossless,
    clippy::single_match_else,
    clippy::struct_excessive_bools,
    clippy::too_many_lines
)]

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
mod restore;
mod rustup;
mod version;

use std::{
    collections::BTreeMap,
    env,
    fmt::{self, Write},
};

use anyhow::{bail, Result};

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

    manifest::with(cx, || {
        if cx.subcommand.is_none() {
            return Ok(());
        }

        let mut progress = Progress::default();
        let packages = determine_package_list(cx, &mut progress)?;
        let mut keep_going = KeepGoing::default();
        if let Some(range) = &cx.version_range {
            let total = progress.total;
            progress.total = 0;
            for (cargo_version, _) in range {
                if cx.target.is_empty() || *cargo_version >= 64 {
                    progress.total += total;
                } else {
                    progress.total += total * cx.target.len();
                }
            }
            let line = cmd!("cargo");

            // First, generate the lockfile using the oldest cargo specified.
            // https://github.com/taiki-e/cargo-hack/issues/105
            let mut generate_lockfile = true;
            // Workaround for spurious "failed to select a version" error.
            // (This does not work around the underlying cargo bug: https://github.com/rust-lang/cargo/issues/10623)
            let mut regenerate_lockfile_on_51_or_up = false;
            for (cargo_version, toolchain) in range {
                rustup::install_toolchain(toolchain, &cx.target, true)?;
                if generate_lockfile || regenerate_lockfile_on_51_or_up && *cargo_version >= 51 {
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
                    generate_lockfile = false;
                    regenerate_lockfile_on_51_or_up = false;
                }
                if *cargo_version < 51 {
                    regenerate_lockfile_on_51_or_up = true;
                }

                if cx.clean_per_version {
                    cargo_clean(cx, None)?;
                }

                let mut line = line.clone();
                line.leading_arg(toolchain);
                line.apply_context(cx);
                exec_on_packages(
                    cx,
                    &packages,
                    line,
                    &mut progress,
                    &mut keep_going,
                    *cargo_version,
                )?;
            }
        } else {
            let mut line = cx.cargo();
            line.apply_context(cx);
            exec_on_packages(
                cx,
                &packages,
                line,
                &mut progress,
                &mut keep_going,
                cx.cargo_version,
            )?;
        }
        if keep_going.count > 0 {
            eprintln!();
            error!("{keep_going}");
        }
        Ok(())
    })
}

#[derive(Default)]
struct Progress {
    total: usize,
    count: usize,
}

enum Kind<'a> {
    Normal,
    Each { features: Vec<&'a Feature> },
    Powerset { features: Vec<Vec<&'a Feature>> },
}

fn determine_kind<'a>(
    cx: &'a Context,
    id: &PackageId,
    progress: &mut Progress,
    multiple_packages: bool,
) -> Option<Kind<'a>> {
    assert!(cx.subcommand.is_some());
    if cx.ignore_private && cx.is_private(id) {
        info!("skipped running on private package `{}`", cx.name_verbose(id));
        return None;
    }
    if !cx.each_feature && !cx.feature_powerset {
        progress.total += 1;
        return Some(Kind::Normal);
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
            Some(Kind::Normal)
        } else {
            progress.total += features.len()
                + !cx.exclude_no_default_features as usize
                + (!cx.exclude_all_features
                    && pkg_features.normal().len() + pkg_features.optional_deps().len() > 1)
                    as usize;
            Some(Kind::Each { features })
        }
    } else if cx.feature_powerset {
        let features =
            features::feature_powerset(features, cx.depth, &cx.at_least_one_of, &package.features);

        if (pkg_features.normal().is_empty() && pkg_features.optional_deps().is_empty()
            || !cx.include_features.is_empty())
            && features.is_empty()
        {
            progress.total += 1;
            Some(Kind::Normal)
        } else {
            progress.total += features.len()
                + !cx.exclude_no_default_features as usize
                + (!cx.exclude_all_features
                    && pkg_features.normal().len() + pkg_features.optional_deps().len() > 1)
                    as usize;
            Some(Kind::Powerset { features })
        }
    } else {
        unreachable!()
    }
}

fn determine_package_list<'a>(
    cx: &'a Context,
    progress: &mut Progress,
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
            .filter_map(|id| {
                determine_kind(cx, id, progress, multiple_packages).map(|kind| (id, kind))
            })
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
            .filter_map(|id| {
                determine_kind(cx, id, progress, multiple_packages).map(|kind| (id, kind))
            })
            .collect()
    } else if cx.current_package().is_none() {
        let multiple_packages = cx.workspace_members().len() > 1;
        cx.workspace_members()
            .filter_map(|id| {
                determine_kind(cx, id, progress, multiple_packages).map(|kind| (id, kind))
            })
            .collect()
    } else {
        let current_package = &cx.packages(cx.current_package().unwrap()).name;
        let multiple_packages = false;
        cx.workspace_members()
            .find(|id| cx.packages(id).name == *current_package)
            .and_then(|id| {
                determine_kind(cx, id, progress, multiple_packages).map(|kind| vec![(id, kind)])
            })
            .unwrap_or_default()
    })
}

fn exec_on_packages(
    cx: &Context,
    packages: &[(&PackageId, Kind<'_>)],
    mut line: ProcessBuilder<'_>,
    progress: &mut Progress,
    keep_going: &mut KeepGoing,
    cargo_version: u32,
) -> Result<()> {
    if cx.target.is_empty() || cargo_version >= 64 {
        // TODO: We should test that cargo's multi-target build does not break the resolver behavior required for a correct check.
        for target in &cx.target {
            line.arg("--target");
            line.arg(target);
        }
        packages
            .iter()
            .try_for_each(|(id, kind)| exec_on_package(cx, id, kind, &line, progress, keep_going))
    } else {
        cx.target.iter().try_for_each(|target| {
            let mut line = line.clone();
            line.arg("--target");
            line.arg(target);
            packages.iter().try_for_each(|(id, kind)| {
                exec_on_package(cx, id, kind, &line, progress, keep_going)
            })
        })
    }
}

fn exec_on_package(
    cx: &Context,
    id: &PackageId,
    kind: &Kind<'_>,
    line: &ProcessBuilder<'_>,
    progress: &mut Progress,
    keep_going: &mut KeepGoing,
) -> Result<()> {
    let package = cx.packages(id);

    let mut line = line.clone();
    line.append_features_from_args(cx, id);

    if !cx.no_manifest_path {
        line.arg("--manifest-path");
        line.arg(
            package.manifest_path.strip_prefix(&cx.current_dir).unwrap_or(&package.manifest_path),
        );
    }

    match kind {
        Kind::Normal => {
            // only run with default features
            return exec_cargo(cx, id, &mut line, progress, keep_going);
        }
        Kind::Each { .. } | Kind::Powerset { .. } => {}
    }

    if !cx.no_default_features {
        line.arg("--no-default-features");
    }

    // if `metadata.packages[].features` has `default` feature, users can
    // specify `--features=default`, so it should be one of the combinations.
    // Otherwise, "run with default features" is basically the same as
    // "run with no default features".

    if !cx.exclude_no_default_features {
        // run with no default features if the package has other features
        exec_cargo(cx, id, &mut line, progress, keep_going)?;
    }

    match kind {
        Kind::Each { features } => {
            for &f in features {
                exec_cargo_with_features(cx, id, &line, progress, keep_going, &[f])?;
            }
        }
        Kind::Powerset { features } => {
            for f in features {
                exec_cargo_with_features(cx, id, &line, progress, keep_going, f)?;
            }
        }
        Kind::Normal => unreachable!(),
    }

    let pkg_features = cx.pkg_features(id);
    if !cx.exclude_all_features
        && pkg_features.normal().len() + pkg_features.optional_deps().len() > 1
    {
        // run with all features
        // https://github.com/taiki-e/cargo-hack/issues/42
        line.arg("--all-features");
        exec_cargo(cx, id, &mut line, progress, keep_going)?;
    }

    Ok(())
}

fn exec_cargo_with_features(
    cx: &Context,
    id: &PackageId,
    line: &ProcessBuilder<'_>,
    progress: &mut Progress,
    keep_going: &mut KeepGoing,
    features: &[&Feature],
) -> Result<()> {
    let mut line = line.clone();
    line.append_features(features);
    exec_cargo(cx, id, &mut line, progress, keep_going)
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
    progress: &mut Progress,
    keep_going: &mut KeepGoing,
) -> Result<()> {
    let res = exec_cargo_inner(cx, id, line, progress);
    if cx.keep_going {
        if let Err(e) = res {
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
    progress: &mut Progress,
) -> Result<()> {
    if progress.count != 0 && !cx.print_command_list {
        eprintln!();
    }
    progress.count += 1;

    if cx.clean_per_run {
        cargo_clean(cx, Some(id))?;
    }

    if cx.print_command_list {
        print_command(line.clone());
        return Ok(());
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

    line.run()
}

fn cargo_clean(cx: &Context, id: Option<&PackageId>) -> Result<()> {
    let mut line = cx.cargo();
    line.arg("clean");
    if let Some(id) = id {
        line.arg("--package");
        line.arg(&cx.packages(id).name);
    }

    if cx.print_command_list {
        print_command(line);
        return Ok(());
    }

    if term::verbose() {
        // running `cargo clean [--package <package>]`
        info!("running {line}");
    }

    line.run()
}

fn print_command(mut line: ProcessBuilder<'_>) {
    let _guard = term::verbose::scoped(true);
    line.strip_program_path = true;
    let l = line.to_string();
    println!("{}", &l[1..l.len() - 1]);
}
