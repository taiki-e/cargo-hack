#![forbid(unsafe_code)]
#![warn(rust_2018_idioms, single_use_lifetimes, unreachable_pub)]
#![warn(clippy::all)]

#[macro_use]
mod term;

mod cli;
mod manifest;
mod metadata;
mod process;

use std::{env, ffi::OsString, fs, path::Path};

use anyhow::{bail, Context, Result};

use crate::{
    cli::{Args, Coloring},
    manifest::{find_root_manifest_for_wd, Manifest},
    metadata::{Metadata, Package},
    process::ProcessBuilder,
};

fn main() {
    let mut coloring = None;
    if let Err(e) = try_main(&mut coloring) {
        error!(coloring, "{:?}", e);
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
            exec_on_package(args, package, &line)?;
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
            exec_on_package(args, package, &line)?;
        }
    } else if current_manifest.is_virtual() {
        for package in &metadata.packages {
            exec_on_package(args, package, &line)?;
        }
    } else if !current_manifest.is_virtual() {
        let current_package = current_manifest.package_name();
        let package =
            metadata.packages.iter().find(|package| package.name == *current_package).unwrap();
        exec_on_package(args, package, &line)?;
    }

    Ok(())
}

fn exec_on_package(args: &Args, package: &Package, line: &ProcessBuilder) -> Result<()> {
    let manifest = Manifest::new(&package.manifest_path)?;

    if args.ignore_private && manifest.is_private() {
        info!(args.color, "skipped running on {}", package.name_verbose(args));
    } else if args.subcommand.is_some() || args.remove_dev_deps {
        no_dev_deps(args, package, manifest, line)?;
    }

    Ok(())
}

fn no_dev_deps(
    args: &Args,
    package: &Package,
    mut manifest: Manifest,
    line: &ProcessBuilder,
) -> Result<()> {
    struct Bomb<'a> {
        manifest: &'a Manifest,
        args: &'a Args,
        done: bool,
        res: &'a mut Result<()>,
    }

    impl Drop for Bomb<'_> {
        fn drop(&mut self) {
            if !self.args.remove_dev_deps {
                let res = fs::write(&self.manifest.path, &self.manifest.raw).with_context(|| {
                    format!("failed to restore manifest file: {}", self.manifest.path.display())
                });

                if self.done {
                    *self.res = res;
                } else if let Err(e) = res {
                    error!(self.args.color, "{:?}", e);
                }
            }
        }
    }

    let f = |args: &Args, package: &Package, line: &ProcessBuilder| {
        let mut line = line.clone();
        line.features(args, package);
        line.arg("--manifest-path");
        line.arg(&package.manifest_path);

        if args.each_feature {
            exec_for_each_feature(args, package, &line)
        } else {
            exec_cargo(args, package, &line)
        }
    };

    if args.no_dev_deps || args.remove_dev_deps {
        let mut res = Ok(());
        let new = manifest.remove_dev_deps();
        let mut bomb = Bomb { manifest: &manifest, args, done: false, res: &mut res };

        fs::write(&package.manifest_path, new).with_context(|| {
            format!("failed to update manifest file: {}", package.manifest_path.display())
        })?;

        if args.subcommand.is_some() {
            f(args, package, line)?;
        }

        bomb.done = true;
        drop(bomb);
        res?;
    } else if args.subcommand.is_some() {
        f(args, package, line)?;
    }

    Ok(())
}

fn exec_for_each_feature(args: &Args, package: &Package, line: &ProcessBuilder) -> Result<()> {
    // run with default features
    exec_cargo(args, package, line)?;

    if package.features.is_empty() {
        return Ok(());
    }

    let mut line = line.clone();
    line.arg("--no-default-features");

    // run with no default features if the package has other features
    //
    // `default` is not skipped because `cfg(feature = "default")` is work
    // if `default` feature specified.
    exec_cargo(args, package, &line)?;

    // run with each feature
    package.features.iter().filter(|(k, _)| *k != "default").try_for_each(|(feature, _)| {
        let mut line = line.clone();
        line.append_features(&[feature]);
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
