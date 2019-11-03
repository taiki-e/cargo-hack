#![forbid(unsafe_code)]
#![warn(rust_2018_idioms, single_use_lifetimes, unreachable_pub)]
#![warn(clippy::all)]

#[macro_use]
mod term;

mod cli;
mod manifest;
mod process;
mod workspace;

use std::{env, ffi::OsString, fs, path::Path};

use anyhow::{bail, Context, Result};

use crate::{
    cli::{Coloring, Options},
    manifest::{find_root_manifest_for_wd, Manifest},
    process::ProcessBuilder,
    workspace::Workspace,
};

fn main() {
    let mut coloring = None;
    if let Err(e) = try_main(&mut coloring) {
        error!(coloring, "{:?}", e);
        std::process::exit(1)
    }
}

fn try_main(coloring: &mut Option<Coloring>) -> Result<()> {
    let args = cli::args(coloring)?;

    if args.first.is_empty()
        || args.subcommand.is_none() && args.first.iter().any(|a| a == "--help" || a == "-h")
    {
        cli::print_help();
        return Ok(());
    }
    if args.first.iter().any(|a| a == "--version" || a == "-V") {
        cli::print_version();
        return Ok(());
    }
    if args.subcommand.is_none() {
        if args.first.iter().any(|a| a == "--list") {
            let mut line = ProcessBuilder::new(cargo_binary());
            line.arg("--list");
            line.exec()?;
            return Ok(());
        } else if !args.remove_dev_deps {
            // TODO: improve this
            bail!(
                "\
No subcommand or valid flag specified.

USAGE:
    cargo hack [OPTIONS] [SUBCOMMAND]

For more information try --help
"
            );
        }
    }

    let current_dir = &env::current_dir()?;

    let current_manifest = match &args.manifest_path {
        Some(path) => Manifest::with_manifest_path(path)?,
        None => Manifest::new(&find_root_manifest_for_wd(&current_dir)?)?,
    };
    let workspace = Workspace::new(&current_manifest)?;

    exec_on_workspace(&args, &workspace)
}

fn exec_on_workspace(args: &Options, workspace: &Workspace<'_>) -> Result<()> {
    if args.workspace.is_some() {
        for spec in &args.exclude {
            if !workspace.members.contains_key(spec) {
                warn!(
                    args.color,
                    "excluded package(s) {} not found in workspace `{}`",
                    spec,
                    workspace.current_manifest.dir().display()
                );
            }
        }

        for (_, manifest) in
            workspace.members.iter().filter(|(spec, _)| !args.exclude.contains(spec))
        {
            exec_on_package(manifest, args)?;
        }
    } else if !args.package.is_empty() {
        for spec in &args.package {
            if let Some(manifest) = workspace.members.get(spec) {
                exec_on_package(manifest, args)?;
            } else {
                bail!("package ID specification `{}` matched no packages", spec);
            }
        }
    } else if workspace.current_manifest.is_virtual() {
        for (_, manifest) in workspace.members.iter() {
            exec_on_package(manifest, args)?;
        }
    } else if !workspace.current_manifest.is_virtual() {
        exec_on_package(workspace.current_manifest, args)?;
    }

    Ok(())
}

fn exec_on_package(manifest: &Manifest, args: &Options) -> Result<()> {
    if args.ignore_private && manifest.is_private() {
        info!(args.color, "skipped running on {}", manifest.package_name_verbose(args));
    } else if args.subcommand.is_some() || args.remove_dev_deps {
        no_dev_deps(manifest, args)?;
    }

    Ok(())
}

fn no_dev_deps(manifest: &Manifest, args: &Options) -> Result<()> {
    struct Bomb<'a> {
        manifest: &'a Manifest,
        args: &'a Options,
        backup_path: &'a Path,
        done: bool,
        res: &'a mut Result<()>,
    }

    impl Drop for Bomb<'_> {
        fn drop(&mut self) {
            let res = (|| {
                if !self.args.remove_dev_deps {
                    fs::write(&self.manifest.path, &self.manifest.raw).with_context(|| {
                        format!("failed to restore manifest file: {}", self.manifest.path.display())
                    })?
                }
                if self.backup_path.exists() {
                    // This will not run if the manifest update fails (early return with above `?`).
                    fs::remove_file(&self.backup_path).with_context(|| {
                        format!("failed to remove backup file: {}", self.backup_path.display())
                    })?
                }
                Ok(())
            })();

            if self.done {
                *self.res = res;
            } else if let Err(e) = res {
                error!(self.args.color, "{:?}", e);
            }
        }
    }

    if args.no_dev_deps || args.remove_dev_deps {
        let backup_path = manifest.path.with_extension("toml.bk");

        let mut res = Ok(());

        let mut bomb =
            Bomb { manifest, args, backup_path: &backup_path, done: false, res: &mut res };

        fs::copy(&manifest.path, &backup_path)
            .with_context(|| format!("failed to create backup file: {}", backup_path.display()))?;

        fs::write(&manifest.path, remove_dev_deps(manifest)).with_context(|| {
            format!("failed to update manifest file: {}", manifest.path.display())
        })?;

        if args.subcommand.is_some() {
            each_feature(manifest, args)?;
        }

        bomb.done = true;
        drop(bomb);
        res?;
    } else if args.subcommand.is_some() {
        each_feature(manifest, args)?;
    }

    Ok(())
}

fn each_feature(manifest: &Manifest, args: &Options) -> Result<()> {
    let mut features = String::new();
    if args.ignore_unknown_features {
        let f: Vec<_> = args
            .features
            .iter()
            .filter(|f| {
                if manifest.features.contains(f) {
                    true
                } else {
                    // ignored
                    info!(
                        args.color,
                        "skipped applying unknown `{}` feature to {}",
                        f,
                        manifest.package_name_verbose(args)
                    );
                    false
                }
            })
            .map(String::as_str)
            .collect();
        if !f.is_empty() {
            features.push_str("--features=");
            features.push_str(&f.join(","));
        }
    } else if !args.features.is_empty() {
        features.push_str("--features=");
        features.push_str(&args.features.join(","));
    }

    let features = if features.is_empty() { None } else { Some(&*features) };

    if args.each_feature {
        exec_each_feature(manifest, args, features)
    } else {
        exec_cargo_command(manifest, args, features, &[])
    }
}

fn remove_dev_deps(manifest: &Manifest) -> String {
    let mut doc = manifest.toml.clone();
    manifest::remove_key_and_target_key(doc.as_table_mut(), "dev-dependencies");
    doc.to_string_in_original_order()
}

fn exec_each_feature(manifest: &Manifest, args: &Options, features: Option<&str>) -> Result<()> {
    // run with default features
    exec_cargo_command(manifest, args, features, &[])?;

    if manifest.features.is_empty() {
        return Ok(());
    }

    // run with no default features if the package has other features
    //
    // `default` is not skipped because `cfg(feature = "default")` is work
    // if `default` feature specified.
    exec_cargo_command(manifest, args, features, &["--no-default-features"])?;

    // run with each feature
    manifest.features.iter().filter(|&k| k != "default").try_for_each(|feature| {
        let features = match features {
            Some(features) => String::from(features) + "," + feature,
            None => String::from("--features=") + feature,
        };
        exec_cargo_command(manifest, args, Some(&*features), &["--no-default-features"])
    })
}

fn exec_cargo_command(
    manifest: &Manifest,
    args: &Options,
    features: Option<&str>,
    extra_args: &[&str],
) -> Result<()> {
    let mut line = ProcessBuilder::new(cargo_binary());

    line.args(&args.first);

    if let Some(features) = features {
        line.arg(features);
    }

    if let Some(target_dir) = args.target_dir.as_ref() {
        line.arg("--target-dir");
        line.arg(target_dir);
    }

    line.args(extra_args);

    line.args2(&args.second);

    if args.verbose {
        line.arg("--manifest-path");
        line.arg(&manifest.path);

        info!(args.color, "running {} on {}", line, manifest.package_name_verbose(args));
    } else {
        info!(args.color, "running {} on {}", line, manifest.package_name_verbose(args));

        // Displaying --manifest-path is redundant.
        line.arg("--manifest-path");
        line.arg(&manifest.path);
    }

    line.exec()
}

fn cargo_binary() -> OsString {
    let cargo_src = env::var_os("CARGO_HACK_CARGO_SRC");
    let cargo = env::var_os("CARGO");
    cargo_src.unwrap_or_else(|| cargo.unwrap_or_else(|| OsString::from("cargo")))
}
