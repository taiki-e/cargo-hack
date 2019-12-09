#![forbid(unsafe_code)]
#![warn(rust_2018_idioms, single_use_lifetimes, unreachable_pub)]
#![warn(clippy::all)]

#[macro_use]
mod term;

mod cli;
mod manifest;
mod metadata;
mod process;
mod remove_dev_deps;
mod restore;

use anyhow::{bail, Context, Error};
use itertools;
use std::{env, ffi::OsString, fs, path::Path};

use crate::{
    cli::{Args, Coloring},
    manifest::{find_root_manifest_for_wd, Manifest},
    metadata::{Metadata, Package},
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
    let args = match cli::args(coloring)? {
        Ok(args) => args,
        Err(code) => std::process::exit(code),
    };

    let metadata = Metadata::new(&args)?;

    let current_manifest = match &args.manifest_path {
        Some(path) => Manifest::new(Path::new(path))?,
        None => Manifest::new(find_root_manifest_for_wd(&env::current_dir()?)?)?,
    };

    exec_on_workspace(&args, &current_manifest, &metadata)
}

fn exec_on_workspace(args: &Args, current_manifest: &Manifest, metadata: &Metadata) -> Result<()> {
    let mut line = ProcessBuilder::from_args(cargo_binary(), &args);

    if let Some(color) = args.color {
        line.arg("--color");
        line.arg(color.as_str());
    }

    let restore = Restore::new(args);

    if args.workspace {
        for spec in &args.exclude {
            if !metadata.packages.iter().any(|package| package.name == *spec) {
                warn!(
                    args.color,
                    "excluded package(s) {} not found in workspace `{}`",
                    spec,
                    metadata.workspace_root.display()
                );
            }
        }

        for package in
            metadata.packages.iter().filter(|package| !args.exclude.contains(&package.name))
        {
            exec_on_package(args, package, &line, &restore)?;
        }
    } else if !args.package.is_empty() {
        for spec in &args.package {
            if !metadata.packages.iter().any(|package| package.name == *spec) {
                bail!("package ID specification `{}` matched no packages", spec);
            }
        }

        for package in
            metadata.packages.iter().filter(|package| args.package.contains(&package.name))
        {
            exec_on_package(args, package, &line, &restore)?;
        }
    } else if current_manifest.is_virtual() {
        for package in &metadata.packages {
            exec_on_package(args, package, &line, &restore)?;
        }
    } else if !current_manifest.is_virtual() {
        let current_package = current_manifest.package_name();
        let package =
            metadata.packages.iter().find(|package| package.name == *current_package).unwrap();
        exec_on_package(args, package, &line, &restore)?;
    }

    Ok(())
}

fn exec_on_package(
    args: &Args,
    package: &Package,
    line: &ProcessBuilder,
    restore: &Restore,
) -> Result<()> {
    let manifest = Manifest::new(&package.manifest_path)?;

    if args.ignore_private && manifest.is_private() {
        info!(args.color, "skipped running on {}", package.name_verbose(args));
    } else if args.subcommand.is_some() || args.remove_dev_deps {
        let mut line = line.clone();
        line.features(args, package);
        line.arg("--manifest-path");
        line.arg(&package.manifest_path);

        no_dev_deps(args, package, &manifest, &line, restore)?;
    }

    Ok(())
}

fn no_dev_deps(
    args: &Args,
    package: &Package,
    manifest: &Manifest,
    line: &ProcessBuilder,
    restore: &Restore,
) -> Result<()> {
    if args.no_dev_deps || args.remove_dev_deps {
        let new = manifest.remove_dev_deps();
        let mut handle = restore.set_manifest(&manifest);

        fs::write(&package.manifest_path, new).with_context(|| {
            format!("failed to update manifest file: {}", package.manifest_path.display())
        })?;

        if args.subcommand.is_some() {
            features(args, package, line)?;
        }

        handle.done()?;
    } else if args.subcommand.is_some() {
        features(args, package, line)?;
    }

    Ok(())
}

fn features(args: &Args, package: &Package, line: &ProcessBuilder) -> Result<()> {
    // run with default features
    exec_cargo(args, package, line)?;

    if (!args.each_feature && !args.feature_powerset) || package.features.is_empty() {
        return Ok(());
    }

    let mut line = line.clone();
    line.arg("--no-default-features");

    // run with no default features if the package has other features
    //
    // `default` is not skipped because `cfg(feature = "default")` is work
    // if `default` feature specified.
    exec_cargo(args, package, &line)?;

    if args.each_feature {
        each_feature(args, package, &line)
    } else if args.feature_powerset {
        feature_powerset(args, package, &line)
    } else {
        Ok(())
    }
}

fn each_feature(args: &Args, package: &Package, line: &ProcessBuilder) -> Result<()> {
    package.features.iter().filter(|(k, _)| *k != "default").try_for_each(|(feature, _)| {
        let mut line = line.clone();
        line.append_features(&[feature]);
        exec_cargo(args, package, &line)
    })
}

fn feature_powerset(args: &Args, package: &Package, line: &ProcessBuilder) -> Result<()> {
    let features: Vec<&String> = package.features.keys().filter(|k| *k != "default").collect();
    let mut powerset = powerset(&features[..]);

    // The first element of a powerset is `[]` so it should be removed.
    powerset.remove(0);

    powerset.into_iter().try_for_each(|elem| {
        let features = itertools::join(elem, ",");
        let mut line = line.clone();
        line.append_features(&[features]);
        exec_cargo(args, package, &line)
    })
}

fn exec_cargo(args: &Args, package: &Package, line: &ProcessBuilder) -> Result<()> {
    info!(args.color, "running {} on {}", line, package.name_verbose(args));
    line.exec()
}

fn cargo_binary() -> OsString {
    env::var_os("CARGO_HACK_CARGO_SRC")
        .unwrap_or_else(|| env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}

fn powerset<T: Clone>(s: &[T]) -> Vec<Vec<T>> {
    s.iter().fold(vec![vec![]], |mut acc, elem| {
        let ext = acc.clone().into_iter().map(|mut curr| {
            curr.push(elem.clone());
            curr
        });
        acc.extend(ext);
        acc
    })
}
