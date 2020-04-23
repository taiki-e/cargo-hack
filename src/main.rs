#![forbid(unsafe_code)]
#![warn(rust_2018_idioms, single_use_lifetimes, unreachable_pub)]
#![warn(clippy::all)]
// mem::take requires Rust 1.40
#![allow(clippy::mem_replace_with_default)]

#[macro_use]
mod term;

mod cli;
mod manifest;
mod metadata;
mod package;
mod process;
mod remove_dev_deps;
mod restore;

use std::{env, ffi::OsString, fs, path::Path};

use anyhow::{bail, Context, Error};

use crate::{
    cli::{Args, Coloring},
    manifest::{find_root_manifest_for_wd, Manifest},
    metadata::Metadata,
    package::{Kind, Package},
    process::ProcessBuilder,
    restore::Restore,
};

type Result<T, E = Error> = std::result::Result<T, E>;

fn main() {
    let mut coloring = None;
    if let Err(e) = try_main(&mut coloring) {
        error!(coloring, "{:#}", e);
        std::process::exit(1)
    }
}

fn try_main(coloring: &mut Option<Coloring>) -> Result<()> {
    let args = cli::args(coloring)?.unwrap_or_else(|| std::process::exit(0));
    let metadata = Metadata::new(&args)?;

    let current_manifest = match &args.manifest_path {
        Some(path) => Manifest::new(Path::new(path))?,
        None => Manifest::new(find_root_manifest_for_wd(&env::current_dir()?)?)?,
    };

    exec_on_workspace(&args, &current_manifest, &metadata)
}

fn exec_on_workspace(args: &Args, current_manifest: &Manifest, metadata: &Metadata) -> Result<()> {
    let restore = Restore::new(args);
    let mut line = ProcessBuilder::from_args(cargo_binary(), &args);

    if let Some(color) = args.color {
        line.arg("--color");
        line.arg(color.as_str());
    }

    let mut total = 0;
    let packages = if args.workspace {
        args.exclude.iter().for_each(|spec| {
            if !metadata.packages.iter().any(|package| package.name == *spec) {
                warn!(
                    args.color,
                    "excluded package(s) {} not found in workspace `{}`",
                    spec,
                    metadata.workspace_root.display()
                );
            }
        });

        let packages =
            metadata.packages.iter().filter(|package| !args.exclude.contains(&package.name));
        Package::from_iter(args, &mut total, packages)?
    } else if !args.package.is_empty() {
        if let Some(spec) = args
            .package
            .iter()
            .find(|spec| !metadata.packages.iter().any(|package| package.name == **spec))
        {
            bail!("package ID specification `{}` matched no packages", spec)
        }

        let packages =
            metadata.packages.iter().filter(|package| args.package.contains(&package.name));
        Package::from_iter(args, &mut total, packages)?
    } else if current_manifest.is_virtual() {
        Package::from_iter(args, &mut total, &metadata.packages)?
    } else {
        let current_package = current_manifest.package_name();
        let package = metadata.packages.iter().find(|package| package.name == *current_package);
        Package::from_iter(args, &mut total, package)?
    };

    let mut info = Info { total, count: 0 };
    packages
        .iter()
        .try_for_each(|package| exec_on_package(args, package, &line, &restore, &mut info))
}

struct Info {
    total: usize,
    count: usize,
}

fn exec_on_package(
    args: &Args,
    package: &Package<'_>,
    line: &ProcessBuilder,
    restore: &Restore,
    info: &mut Info,
) -> Result<()> {
    if let Kind::Skip = package.kind {
        info!(args.color, "skipped running on {}", package.name_verbose(args));
    } else if args.subcommand.is_some() || args.remove_dev_deps {
        let mut line = line.clone();
        line.features(args, package);
        line.arg("--manifest-path");
        line.arg(&package.manifest_path);

        no_dev_deps(args, package, &line, restore, info)?;
    }

    Ok(())
}

fn no_dev_deps(
    args: &Args,
    package: &Package<'_>,
    line: &ProcessBuilder,
    restore: &Restore,
    info: &mut Info,
) -> Result<()> {
    if args.no_dev_deps || args.remove_dev_deps {
        let new = package.manifest.remove_dev_deps();
        let mut handle = restore.set_manifest(&package.manifest);

        fs::write(&package.manifest_path, new).with_context(|| {
            format!("failed to update manifest file: {}", package.manifest_path.display())
        })?;

        if args.subcommand.is_some() {
            package::features(args, package, line, info)?;
        }

        handle.done()?;
    } else if args.subcommand.is_some() {
        package::features(args, package, line, info)?;
    }

    Ok(())
}

fn cargo_binary() -> OsString {
    env::var_os("CARGO_HACK_CARGO_SRC")
        .unwrap_or_else(|| env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}
