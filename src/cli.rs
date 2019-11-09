use std::{env, fs, path::PathBuf, str::FromStr};

use anyhow::{bail, format_err, Error, Result};
use termcolor::ColorChoice;

pub(crate) fn print_version() {
    println!("{0} {1}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"),)
}

pub(crate) fn print_help() {
    println!(
        "\
{0} {1}
{2}

{3}
USAGE:
    cargo hack [OPTIONS] [SUBCOMMAND]

OPTIONS:
    -p, --package <SPEC>...         Package(s) to check
                                    (this flag will not be propagated to cargo)
        --all                       Alias for --workspace
        --workspace                 Perform command for all packages in the
                                    workspace
                                    (this flag will not be propagated to cargo)
        --exclude <SPEC>...         Exclude packages from the check
                                    (this flag will not be propagated to cargo)
        --target-dir <DIRECTORY>    Directory for all generated artifacts
                                    (this flag will be passed to cargo after
                                    normalizing the given path)
        --manifest-path <PATH>      Path to Cargo.toml
                                    (this flag will not be propagated to cargo)
        --features <FEATURES>...    Space-separated list of features to activate
        --each-feature              Perform for each feature which includes
                                    `--no-default-features` and default features
                                    of the package
        --no-dev-deps               Perform without dev-dependencies
        --remove-dev-deps           Equivalent to `--no-dev-deps` except for
                                    does not restore the original `Cargo.toml`
                                    after execution
        --ignore-private            Skip to perform on `publish = false` packages
        --ignore-unknown-features   Skip passing `--features` to `cargo` if that
                                    feature does not exist in the package.
    -v, --verbose                   Use verbose output
                                    (this flag will be propagated to cargo)
        --color <WHEN>              Coloring: auto, always, never
    -h, --help                      Prints help information
    -V, --version                   Prints version information

Some common cargo commands are (see all commands with --list):
    build       Compile the current package
    check       Analyze the current package and report errors, but don't build object files
    run         Run a binary or example of the local package
    test        Run the tests
",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_AUTHORS"),
        env!("CARGO_PKG_DESCRIPTION"),
    )
}

#[derive(Debug)]
pub(crate) struct Args {
    pub(crate) first: Vec<String>,
    pub(crate) second: Vec<String>,

    pub(crate) subcommand: Option<String>,

    pub(crate) manifest_path: Option<String>,
    // canonicalized target-dir
    pub(crate) target_dir: Option<PathBuf>,

    pub(crate) package: Vec<String>,
    pub(crate) exclude: Vec<String>,
    pub(crate) features: Vec<String>,

    pub(crate) workspace: Option<String>,
    pub(crate) each_feature: bool,
    pub(crate) no_dev_deps: bool,
    pub(crate) remove_dev_deps: bool,
    pub(crate) ignore_private: bool,
    pub(crate) ignore_unknown_features: bool,

    pub(crate) color: Option<Coloring>,
    pub(crate) verbose: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Coloring {
    Auto,
    Always,
    Never,
}

impl Coloring {
    pub(crate) fn color_choice(color: Option<Coloring>) -> ColorChoice {
        match color {
            Some(Coloring::Auto) | None => ColorChoice::Auto,
            Some(Coloring::Always) => ColorChoice::Always,
            Some(Coloring::Never) => ColorChoice::Never,
        }
    }
}

impl FromStr for Coloring {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self> {
        match name {
            "auto" => Ok(Coloring::Auto),
            "always" => Ok(Coloring::Always),
            "never" => Ok(Coloring::Never),
            other => bail!("must be auto, always, or never, but found `{}`", other),
        }
    }
}

#[allow(clippy::cognitive_complexity)]
pub(crate) fn args(coloring: &mut Option<Coloring>) -> Result<Option<Args>> {
    let mut args = env::args();
    let _ = args.next(); // executable name
    match &args.next() {
        Some(a) if a == "hack" => {}
        Some(_) | None => return Ok(None),
    }

    let mut first = Vec::new();

    let mut subcommand: Option<String> = None;
    let mut target_dir = None;
    let mut manifest_path = None;
    let mut color = None;

    let mut package = Vec::new();
    let mut exclude = Vec::new();
    let mut features = Vec::new();

    let mut workspace = None;
    let mut no_dev_deps = false;
    let mut remove_dev_deps = false;
    let mut each_feature = false;
    let mut ignore_private = false;
    let mut ignore_unknown_features = false;
    let mut ignore_non_exist_features = false;

    let res = (|| -> Result<()> {
        while let Some(arg) = args.next() {
            // stop at `--`
            // 1. `cargo hack check --no-dev-deps`
            //   first:  `cargo hack check --no-dev-deps` (filtered and passed to `cargo`)
            //   second: (empty)
            // 2. `cargo hack test --each-feature -- --ignored`
            //   first:  `cargo hack test --each-feature` (filtered and passed to `cargo`)
            //   second: `--ignored` (passed directly to `cargo` with `--`)
            if arg == "--" {
                break;
            }

            if !arg.starts_with('-') {
                if subcommand.is_none() {
                    subcommand = Some(arg.clone());
                }
                first.push(arg);
                continue;
            }

            macro_rules! parse_arg1 {
                ($ident:ident, $propagate:expr, $pat1:expr, $pat2:expr, $help:expr) => {
                    if arg == $pat1 {
                        if $ident.is_some() {
                            return Err(multi_arg($help, subcommand.as_ref()));
                        }
                        match args.next() {
                            None => return Err(req_arg($help, subcommand.as_ref())),
                            Some(next) => {
                                if $propagate {
                                    $ident = Some(next.clone());
                                    first.push(arg);
                                    first.push(next);
                                } else {
                                    $ident = Some(next);
                                }
                            }
                        }
                        continue;
                    } else if arg.starts_with($pat2) {
                        if $ident.is_some() {
                            return Err(multi_arg($help, subcommand.as_ref()));
                        }
                        match arg.splitn(2, '=').nth(1).map(|s| s.to_string()) {
                            None => return Err(req_arg($help, subcommand.as_ref())),
                            arg @ Some(_) => $ident = arg,
                        }
                        if $propagate {
                            first.push(arg);
                        }
                        continue;
                    }
                };
            }
            macro_rules! parse_arg2 {
                ($ident:ident, $allow_split:expr, $pat1:expr, $pat2:expr, $help:expr) => {
                    if arg == $pat1 {
                        if let Some(arg) = args.next() {
                            if $allow_split {
                                $ident.extend(arg.split(',').map(|s| s.to_string()));
                            } else {
                                $ident.push(arg);
                            }
                        } else {
                            return Err(req_arg($help, subcommand.as_ref()));
                        }
                        continue;
                    } else if arg.starts_with($pat2) {
                        if let Some(arg) = arg.splitn(2, '=').nth(1) {
                            if $allow_split {
                                $ident.extend(arg.split(',').map(|s| s.to_string()));
                            } else {
                                $ident.push(arg.to_string());
                            }
                        } else {
                            return Err(req_arg($help, subcommand.as_ref()));
                        }
                        continue;
                    }
                };
            }

            parse_arg1!(
                target_dir,
                false,
                "--target-dir",
                "--target-dir=",
                "--target-dir <DIRECTORY>"
            );
            parse_arg1!(
                manifest_path,
                false,
                "--manifest-path",
                "--manifest-path=",
                "--manifest-path <PATH>"
            );
            parse_arg1!(color, true, "--color", "--color=", "--color <WHEN>");

            parse_arg2!(package, false, "--package", "--package=", "--package <SPEC>");
            parse_arg2!(package, false, "-p", "-p=", "--package <SPEC>");
            parse_arg2!(exclude, false, "--exclude", "--exclude=", "--exclude <SPEC>");
            parse_arg2!(features, true, "--features", "--features=", "--features <FEATURES>");

            match arg.as_str() {
                "--workspace" | "--all" => {
                    if workspace.is_some() {
                        return Err(multi_arg(&arg, subcommand.as_ref()));
                    }
                    workspace = Some(arg);
                }
                "--no-dev-deps" => {
                    if no_dev_deps {
                        return Err(multi_arg(&arg, subcommand.as_ref()));
                    }
                    no_dev_deps = true;
                }
                "--remove-dev-deps" => {
                    if remove_dev_deps {
                        return Err(multi_arg(&arg, subcommand.as_ref()));
                    }
                    remove_dev_deps = true;
                }
                "--each-feature" => {
                    if each_feature {
                        return Err(multi_arg(&arg, subcommand.as_ref()));
                    }
                    each_feature = true;
                }
                "--ignore-private" => {
                    if ignore_private {
                        return Err(multi_arg(&arg, subcommand.as_ref()));
                    }
                    ignore_private = true;
                }
                "--ignore-unknown-features" => {
                    if ignore_unknown_features || ignore_non_exist_features {
                        return Err(multi_arg(&arg, subcommand.as_ref()));
                    }
                    ignore_unknown_features = true;
                }
                "--ignore-non-exist-features" => {
                    if ignore_unknown_features || ignore_non_exist_features {
                        return Err(multi_arg("--ignore-unknown-features", subcommand.as_ref()));
                    }
                    ignore_non_exist_features = true;
                }
                _ => first.push(arg),
            }
        }

        Ok(())
    })();

    let color = color.map(|c| c.parse()).transpose()?;
    *coloring = color;

    if first.is_empty() && !remove_dev_deps
        || subcommand.is_none() && first.iter().any(|a| a == "--help" || a == "-h")
    {
        return Ok(None);
    }

    let target_dir = target_dir.map(fs::canonicalize).transpose()?;
    let verbose = first.iter().any(|a| a == "--verbose" || a == "-v" || a == "-vv");
    if ignore_non_exist_features {
        warn!(
            color,
            "'--ignore-non-exist-features' flag is deprecated, use '--ignore-unknown-features' flag instead"
        );
    }
    if !exclude.is_empty() && workspace.is_none() {
        bail!("--exclude can only be used together with --workspace")
    }
    if let Some(subcommand) = &subcommand {
        if subcommand == "test" || subcommand == "bench" {
            if remove_dev_deps {
                bail!("--remove-dev-deps may not be used together with {} subcommand", subcommand)
            } else if no_dev_deps {
                bail!("--no-dev-deps may not be used together with {} subcommand", subcommand)
            }
        }
    }
    if let Some(pos) = first.iter().position(|a| match a.as_str() {
        "--example" | "--examples" | "--test" | "--tests" | "--bench" | "--benches"
        | "--all-targets" => true,
        _ => false,
    }) {
        if remove_dev_deps {
            bail!("--remove-dev-deps may not be used together with {}", first[pos])
        } else if no_dev_deps {
            bail!("--no-dev-deps may not be used together with {}", first[pos])
        }
    }

    res.map(|()| {
        Some(Args {
            first,
            second: args.collect(),

            subcommand,
            manifest_path,
            target_dir,

            package,
            exclude,
            features,

            workspace,
            each_feature,
            no_dev_deps,
            remove_dev_deps,
            ignore_private,
            ignore_unknown_features: ignore_unknown_features || ignore_non_exist_features,

            color,
            verbose,
        })
    })
}

fn req_arg(arg: &str, subcommand: Option<&String>) -> Error {
    format_err!(
        "\
The argument '{0}' requires a value but none was supplied

USAGE:
    cargo hack{1} {0}

For more information try --help
",
        arg,
        if let Some(subcommand) = subcommand {
            String::from(" ") + subcommand
        } else {
            String::from("")
        }
    )
}

fn multi_arg(arg: &str, subcommand: Option<&String>) -> Error {
    format_err!(
        "\
The argument '{0}' was provided more than once, but cannot be used multiple times

USAGE:
    cargo hack{1} {0}

For more information try --help
",
        arg,
        if let Some(subcommand) = subcommand {
            String::from(" ") + subcommand
        } else {
            String::from("")
        }
    )
}
