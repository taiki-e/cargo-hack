#![forbid(unsafe_code)]
#![warn(rust_2018_idioms, single_use_lifetimes, unreachable_pub)]
#![warn(clippy::all)]

use std::{env, ffi::OsString, fmt, fs, io::Write, path::Path, process::Command};

use termcolor::{Color, ColorSpec, StandardStream, WriteColor};

use crate::{
    cli::{Args, Coloring},
    error::Result,
    manifest::Manifest,
};

#[macro_use]
mod error;

mod cli;
mod manifest;

fn main() {
    let mut coloring = None;
    if let Err(e) = try_main(&mut coloring) {
        if e.downcast_ref::<error::StringError>().map_or(true, |e| !e.0.is_empty()) {
            print_error(coloring, &e.to_string());
        }
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
            exec_cargo(Command::new(&args.binary).arg("--list"))?;
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
            )
        }
    }

    if let Some(flag) = &args.workspace {
        print_warning(args.color, &format!("`{}` flag for `cargo hack` is experimental", flag))
    }
    if !args.package.is_empty() {
        print_warning(args.color, "`--package` flag for `cargo hack` is currently ignored")
    }
    if !args.exclude.is_empty() {
        print_warning(args.color, "`--exclude` flag for `cargo hack` is currently ignored")
    }

    let current_dir = &env::current_dir()?;

    let mut root_manifest = match &args.manifest_path {
        Some(path) => Manifest::with_manifest_path(path)?,
        None => Manifest::new(&manifest::find_root_manifest_for_wd(&current_dir)?)?,
    };

    exec_on_workspace(&args, &mut root_manifest)
}

fn exec_on_workspace(args: &Args, root_manifest: &mut Manifest) -> Result<()> {
    let root_dir = root_manifest.dir().to_path_buf();

    if args.workspace.is_some() || root_manifest.is_virtual() {
        root_manifest
            .members()
            .into_iter()
            .flat_map(|v| v.iter().filter_map(|v| v.as_str()))
            .try_for_each(|dir| {
                let path = manifest::find_project_manifest_exact(&root_dir.join(dir))?;
                let mut manifest = crate::Manifest::new(&path)?;

                if root_manifest.path == manifest.path {
                    return Ok(());
                }

                exec_on_package(&mut manifest, args)
            })?;
    }

    if !root_manifest.is_virtual() {
        exec_on_package(root_manifest, args)?;
    }

    Ok(())
}

fn exec_on_package(manifest: &mut Manifest, args: &Args) -> Result<()> {
    if args.ignore_private && manifest.is_private() {
        print_info(
            args.color,
            &format!("skipped performing on {}", manifest.package_name_verbose(args)),
        );
        Ok(())
    } else if args.subcommand.is_some() {
        no_dev_deps(manifest, args, each_feature)
    } else if args.remove_dev_deps {
        no_dev_deps(manifest, args, |_, _| Ok(()))
    } else {
        Ok(())
    }
}

fn no_dev_deps(
    manifest: &mut Manifest,
    args: &Args,
    run_cargo: fn(manifest: &Manifest, args: &Args) -> Result<()>,
) -> Result<()> {
    struct Bomb<'a> {
        path: &'a Path,
        backup_path: &'a Path,
        manifest: &'a str,
        remove_dev_deps: bool,
        flag: bool,
    }

    impl Drop for Bomb<'_> {
        fn drop(&mut self) {
            if self.flag {
                if !self.remove_dev_deps {
                    let _ = fs::write(self.path, self.manifest);
                }

                let _ = fs::remove_file(self.backup_path);
            }
        }
    }

    if args.no_dev_deps || args.remove_dev_deps {
        let backup_path = manifest.path.with_extension("toml.bk");

        // backup
        fs::copy(&manifest.path, &backup_path)?;

        fs::write(&manifest.path, remove_dev_deps(manifest)?)?;

        let mut _bomb = Bomb {
            path: &manifest.path,
            backup_path: &backup_path,
            manifest: &manifest.raw,
            remove_dev_deps: args.remove_dev_deps,
            flag: true,
        };

        run_cargo(manifest, args)?;

        _bomb.flag = false;

        if !args.remove_dev_deps {
            // restore backup
            fs::write(&manifest.path, &manifest.raw)?;
        }

        // remove backup
        fs::remove_file(&backup_path)?;

        Ok(())
    } else {
        run_cargo(manifest, args)
    }
}

fn each_feature(manifest: &Manifest, args: &Args) -> Result<()> {
    let mut features = String::new();
    if args.ignore_non_exist_features {
        let f: Vec<_> = args
            .features
            .iter()
            .filter(|f| {
                if manifest.features.contains(f) {
                    true
                } else {
                    // ignored
                    print_info(
                        args.color,
                        &format!(
                            "skipped applying non-exist `{}` feature to {}",
                            f,
                            manifest.package_name_verbose(args)
                        ),
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

fn remove_dev_deps(manifest: &Manifest) -> Result<String> {
    let mut doc = manifest.toml.clone();
    manifest::remove_key_and_target_key(doc.as_table_mut(), "dev-dependencies");
    Ok(doc.to_string_in_original_order())
}

fn exec_each_feature(manifest: &Manifest, args: &Args, features: Option<&str>) -> Result<()> {
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

struct CmdFormater<'a> {
    args: &'a Args,
    features: Option<&'a str>,
    extra_args: &'a [&'a str],
}

impl fmt::Display for CmdFormater<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Path::new(&self.args.binary).file_stem().unwrap().to_string_lossy())?;
        self.args.first.iter().try_for_each(|arg| write!(f, " {}", arg))?;
        if let Some(features) = self.features {
            write!(f, " {}", features)?;
        }
        self.extra_args.iter().try_for_each(|arg| write!(f, " {}", arg))?;
        self.args.second.iter().try_for_each(|arg| write!(f, " {}", arg))
    }
}

fn exec_cargo_command(
    manifest: &Manifest,
    args: &Args,
    features: Option<&str>,
    extra_args: &[&str],
) -> Result<()> {
    print_info(
        args.color,
        &format!(
            "performing `{}` on {}",
            CmdFormater { args, features, extra_args },
            manifest.package_name_verbose(args)
        ),
    );

    let mut cmd = Command::new(&args.binary);
    cmd.args(&args.first);
    cmd.args(features);

    cmd.args(extra_args).args(&args.second);
    exec_cargo(cmd.current_dir(manifest.dir()))
}

fn exec_cargo(cmd: &mut Command) -> Result<()> {
    let exit_status =
        cmd.spawn().expect("could not run cargo").wait().expect("failed to wait for cargo?");
    if !exit_status.success() { bail!("") } else { Ok(()) }
}

fn cargo_binary() -> OsString {
    let cargo_src = env::var_os("CARGO_HACK_CARGO_SRC");
    let cargo = env::var_os("CARGO");
    cargo_src.unwrap_or_else(|| cargo.unwrap_or_else(|| OsString::from("cargo")))
}

fn print_error(coloring: Option<Coloring>, msg: &str) {
    print_inner(coloring, Some(Color::Red), "error", msg);
}

fn print_warning(coloring: Option<Coloring>, msg: &str) {
    print_inner(coloring, Some(Color::Yellow), "warning", msg);
}

fn print_info(coloring: Option<Coloring>, msg: &str) {
    print_inner(coloring, None, "info", msg);
}

fn print_inner(coloring: Option<Coloring>, color: Option<Color>, kind: &str, msg: &str) {
    let mut stream = StandardStream::stderr(Coloring::color_choice(coloring));
    let _ = stream.set_color(ColorSpec::new().set_bold(true).set_fg(color));
    let _ = write!(stream, "{}", kind);
    let _ = stream.reset();
    if !msg.is_empty() {
        let _ = writeln!(stream, ": {}", msg);
    }
}
