// SPDX-License-Identifier: Apache-2.0 OR MIT

#![forbid(unsafe_code)]

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
    collections::{BTreeMap, HashSet},
    env,
    fmt::{self, Write},
    str::FromStr,
};

use anyhow::{bail, format_err, Error, Result};

use crate::{
    context::Context,
    features::Feature,
    metadata::PackageId,
    process::ProcessBuilder,
    rustup::Rustup,
    version::{Version, VersionRange},
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

        let packages = determine_package_list(cx)?;
        let mut progress = Progress::default();
        let mut keep_going = KeepGoing::default();
        if let Some(range) = cx.version_range {
            let mut versions = BTreeMap::new();
            let steps = rustup::version_range(range, cx.version_step, &packages, cx)?;
            for pkg in packages {
                let msrv = cx
                    .rust_version(pkg.id)
                    .map(str::parse::<Version>)
                    .transpose()?
                    .map(Version::strip_patch);
                if range == VersionRange::msrv() {
                    let msrv = msrv.ok_or_else(|| {
                        format_err!(
                            "no rust-version field in {}'s Cargo.toml is specified",
                            cx.packages(pkg.id).name
                        )
                    })?;
                    versions.entry(msrv).or_insert_with(Vec::new).push(pkg);
                } else {
                    let mut seen = false;
                    for cargo_version in &steps {
                        if msrv.is_some() && Some(*cargo_version) < msrv {
                            continue;
                        }
                        if !seen {
                            if Some(*cargo_version) != msrv {
                                if let Some(msrv) = msrv {
                                    versions.entry(msrv).or_insert_with(Vec::new).push(pkg.clone());
                                }
                            }
                            seen = true;
                        }
                        versions.entry(*cargo_version).or_insert_with(Vec::new).push(pkg.clone());
                    }
                    if !seen {
                        let package = cx.packages(pkg.id);
                        let name = &package.name;
                        let msrv = msrv.expect("always `seen` if no msrv");
                        warn!("skipping {name}, rust-version ({msrv}) is not in specified range ({range})");
                    }
                }
            }

            for (cargo_version, packages) in &versions {
                for package in packages {
                    if cx.target.is_empty() || cargo_version.minor >= 64 {
                        progress.total += package.feature_count;
                    } else {
                        progress.total += package.feature_count * cx.target.len();
                    }
                }
            }

            // First, generate the lockfile using the oldest cargo specified.
            // https://github.com/taiki-e/cargo-hack/issues/105
            let mut generate_lockfile = !cx.locked;
            // Workaround for spurious "failed to select a version" error.
            // (This does not work around the underlying cargo bug: https://github.com/rust-lang/cargo/issues/10623)
            let mut regenerate_lockfile_on_51_or_up = false;
            for (cargo_version, packages) in versions {
                versioned_cargo_exec_on_packages(
                    cx,
                    &packages,
                    cargo_version.minor,
                    &mut progress,
                    &mut keep_going,
                    &mut generate_lockfile,
                    &mut regenerate_lockfile_on_51_or_up,
                )?;
            }
        } else {
            let total = packages.iter().map(|p| p.feature_count).sum();
            progress.total = total;
            default_cargo_exec_on_packages(cx, &packages, &mut progress, &mut keep_going)?;
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

#[derive(Clone)]
enum Kind<'a> {
    Normal,
    Each { features: Vec<&'a Feature> },
    Powerset { features: Vec<Vec<&'a Feature>> },
}

fn determine_kind<'a>(
    cx: &'a Context,
    id: &'a PackageId,
    multiple_packages: bool,
) -> Option<PackageRuns<'a>> {
    assert!(cx.subcommand.is_some());
    if cx.ignore_private && cx.is_private(id) {
        info!("skipped running on private package `{}`", cx.name_verbose(id));
        return None;
    }
    if !cx.each_feature && !cx.feature_powerset {
        let feature_count = 1;
        let kind = Kind::Normal;
        return Some(PackageRuns { id, kind, feature_count });
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
            if cx.ignore_unknown_features {
                let all_valid_features: HashSet<_> = pkg_features
                    .normal()
                    .iter()
                    .chain(pkg_features.optional_deps())
                    .flat_map(|f| f.as_group())
                    .map(|f| f.as_str())
                    .collect();
                features.extend(cx.group_features.iter().filter(|f| {
                    f.as_group().iter().all(|f| all_valid_features.contains(f.as_str()))
                }));
            } else {
                features.extend(cx.group_features.iter());
            }
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
            let feature_count = 1;
            let kind = Kind::Normal;
            Some(PackageRuns { id, kind, feature_count })
        } else {
            let feature_count = features.len()
                + usize::from(!cx.exclude_no_default_features)
                + usize::from(
                    !cx.exclude_all_features
                        && pkg_features.normal().len() + pkg_features.optional_deps().len() > 1,
                );
            let kind = Kind::Each { features };
            Some(PackageRuns { id, kind, feature_count })
        }
    } else if cx.feature_powerset {
        let features = features::feature_powerset(
            features,
            cx.depth,
            &cx.at_least_one_of,
            &cx.mutually_exclusive_features,
            &package.features,
        );

        if (pkg_features.normal().is_empty() && pkg_features.optional_deps().is_empty()
            || !cx.include_features.is_empty())
            && features.is_empty()
        {
            let feature_count = 1;
            let kind = Kind::Normal;
            Some(PackageRuns { id, kind, feature_count })
        } else {
            let feature_count = features.len()
                + usize::from(!cx.exclude_no_default_features)
                + usize::from(
                    !cx.exclude_all_features
                        && pkg_features.normal().len() + pkg_features.optional_deps().len() > 1,
                );
            let kind = Kind::Powerset { features };
            Some(PackageRuns { id, kind, feature_count })
        }
    } else {
        unreachable!()
    }
}

#[derive(Clone)]
struct PackageRuns<'a> {
    id: &'a PackageId,
    kind: Kind<'a>,
    feature_count: usize,
}

fn determine_package_list(cx: &Context) -> Result<Vec<PackageRuns<'_>>> {
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
            .filter_map(|id| determine_kind(cx, id, multiple_packages))
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
            .filter_map(|id| determine_kind(cx, id, multiple_packages))
            .collect()
    } else if cx.current_package().is_none() {
        let multiple_packages = cx.workspace_members().len() > 1;
        cx.workspace_members().filter_map(|id| determine_kind(cx, id, multiple_packages)).collect()
    } else {
        let current_package = &cx.packages(cx.current_package().unwrap()).name;
        let multiple_packages = false;
        cx.workspace_members()
            .find(|id| cx.packages(id).name == *current_package)
            .and_then(|id| determine_kind(cx, id, multiple_packages).map(|p| vec![p]))
            .unwrap_or_default()
    })
}

fn versioned_cargo_exec_on_packages(
    cx: &Context,
    packages: &[PackageRuns<'_>],
    cargo_version: u32,
    progress: &mut Progress,
    keep_going: &mut KeepGoing,
    generate_lockfile: &mut bool,
    regenerate_lockfile_on_51_or_up: &mut bool,
) -> Result<()> {
    // Do not use `cargo +<toolchain>` due to a rustup bug: https://github.com/rust-lang/rustup/issues/3036
    let mut line = cmd!("rustup");
    line.leading_arg("run");

    let toolchain = format!("1.{cargo_version}");
    let print_output = true;
    rustup::install_toolchain(&toolchain, &cx.target, print_output, cx.log_group)?;
    if *generate_lockfile || *regenerate_lockfile_on_51_or_up && cargo_version >= 51 {
        let mut line = line.clone();
        line.leading_arg(&toolchain);
        line.leading_arg("cargo");
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
        *generate_lockfile = false;
        *regenerate_lockfile_on_51_or_up = false;
    }
    if cargo_version < 51 {
        *regenerate_lockfile_on_51_or_up = true;
    }

    if cx.clean_per_version {
        cargo_clean(cx, None)?;
    }

    let mut line = line.clone();
    line.leading_arg(&toolchain);
    line.leading_arg("cargo");
    line.apply_context(cx);
    exec_on_packages(cx, packages, line, progress, keep_going, cargo_version)
}

fn default_cargo_exec_on_packages(
    cx: &Context,
    packages: &[PackageRuns<'_>],
    progress: &mut Progress,
    keep_going: &mut KeepGoing,
) -> Result<()> {
    let mut line = cx.cargo();
    line.apply_context(cx);
    exec_on_packages(cx, packages, line, progress, keep_going, cx.cargo_version)
}

fn exec_on_packages(
    cx: &Context,
    packages: &[PackageRuns<'_>],
    mut line: ProcessBuilder<'_>,
    progress: &mut Progress,
    keep_going: &mut KeepGoing,
    cargo_version: u32,
) -> Result<()> {
    if cx.locked {
        line.arg("--locked");
    }
    if cx.target.is_empty() || cargo_version >= 64 {
        // TODO: We should test that cargo's multi-target build does not break the resolver behavior required for a correct check.
        for target in &cx.target {
            line.arg("--target");
            line.arg(target);
        }
        packages
            .iter()
            .try_for_each(|pkg| exec_on_package(cx, pkg.id, &pkg.kind, &line, progress, keep_going))
    } else {
        cx.target.iter().try_for_each(|target| {
            let mut line = line.clone();
            line.arg("--target");
            line.arg(target);
            packages.iter().try_for_each(|pkg| {
                exec_on_package(cx, pkg.id, &pkg.kind, &line, progress, keep_going)
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
        writeln!(f, "failed to run {} commands\n", self.count)?;
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

#[derive(Clone, Copy, PartialEq)]
enum LogGroup {
    None,
    GithubActions,
}

impl LogGroup {
    fn auto() -> Self {
        if env::var_os("GITHUB_ACTIONS").is_some() {
            Self::GithubActions
        } else {
            Self::None
        }
    }

    fn print(self, msg: &str) -> Option<LogGroupGuard> {
        match self {
            Self::GithubActions => {
                println!("::group::{msg}");
                Some(LogGroupGuard)
            }
            Self::None => {
                info!("{msg}");
                None
            }
        }
    }
}

struct LogGroupGuard;
impl Drop for LogGroupGuard {
    fn drop(&mut self) {
        println!("::endgroup::");
    }
}

impl FromStr for LogGroup {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "github-actions" => Ok(Self::GithubActions),
            other => bail!(
                "argument for --log-group must be none or github-actions, but found `{other}`"
            ),
        }
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
    if progress.count != 0 && !cx.print_command_list && cx.log_group == LogGroup::None {
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
    let _guard = cx.log_group.print(&msg);

    line.run()
}

fn cargo_clean(cx: &Context, id: Option<&PackageId>) -> Result<()> {
    let mut line = cx.cargo();
    line.arg("clean");
    if cx.locked {
        line.arg("--locked");
    }
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
