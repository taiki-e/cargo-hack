use anyhow::{bail, format_err, Error};
use std::{env, fmt, mem, str::FromStr};
use termcolor::ColorChoice;

use crate::{ProcessBuilder, Result};

fn print_version() {
    println!("{0} {1}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}

// (short flag, long flag, short descriptions, additional descriptions)
const HELP: &[(&str, &str, &str, &[&str])] = &[
    ("-p", "--package <SPEC>...", "Package(s) to check", &[]),
    ("", "--all", "Alias for --workspace", &[]),
    ("", "--workspace", "Perform command for all packages in the workspace", &[]),
    ("", "--exclude <SPEC>...", "Exclude packages from the check", &[
        "This flag can only be used together with --workspace",
    ]),
    ("", "--manifest-path <PATH>", "Path to Cargo.toml", &[]),
    ("", "--features <FEATURES>...", "Space-separated list of features to activate", &[]),
    ("", "--each-feature", "Perform for each feature of the package", &[
        "This also includes runs with just --no-default-features flag, --all-features flag, and default features.",
    ]),
    ("", "--feature-powerset", "Perform for the feature powerset of the package", &[
        "This also includes runs with just --no-default-features flag, --all-features flag, and default features.",
    ]),
    ("", "--optional-deps [DEPS]...", "Use optional dependencies as features", &[
        "If DEPS are not specified, all optional dependencies are considered as features.",
        "This flag can only be used together with either --each-feature flag or --feature-powerset flag.",
    ]),
    ("", "--skip <FEATURES>...", "Alias for --exclude-features", &[]),
    ("", "--exclude-features <FEATURES>...", "Space-separated list of features to exclude", &[
        "To exclude run of default feature, using value `--exclude-features default`.",
        "To exclude run of just --no-default-features flag, using --exclude-no-default-features flag.",
        "To exclude run of just --all-features flag, using --exclude-all-features flag.",
        "This flag can only be used together with either --each-feature flag or --feature-powerset flag.",
    ]),
    ("", "--exclude-no-default-features", "Exclude run of just --no-default-features flag", &[
        "This flag can only be used together with either --each-feature flag or --feature-powerset flag.",
    ]),
    ("", "--exclude-all-features", "Exclude run of just --all-features flag", &[
        "This flag can only be used together with either --each-feature flag or --feature-powerset flag.",
    ]),
    (
        "",
        "--depth <NUM>",
        "Specify a max number of simultaneous feature flags of --feature-powerset",
        &[
            "If NUM is set to 1, --feature-powerset is equivalent to --each-feature.",
            "This flag can only be used together with --feature-powerset flag.",
        ],
    ),
    (
        "",
        "--include-features <FEATURES>...",
        "Include only the specified features in the feature combinations instead of package features",
        &[
            "This flag can only be used together with either --each-feature flag or --feature-powerset flag.",
        ],
    ),
    ("", "--no-dev-deps", "Perform without dev-dependencies", &[
        "Note that this flag removes dev-dependencies from real `Cargo.toml` while cargo-hack is running and restores it when finished.",
    ]),
    (
        "",
        "--remove-dev-deps",
        "Equivalent to --no-dev-deps flag except for does not restore the original `Cargo.toml` after performed",
        &[],
    ),
    ("", "--ignore-private", "Skip to perform on `publish = false` packages", &[]),
    (
        "",
        "--ignore-unknown-features",
        "Skip passing --features flag to `cargo` if that feature does not exist in the package",
        &["This flag can only be used together with either --features or --include-features."],
    ),
    ("", "--clean-per-run", "Remove artifacts for that package before running the command", &[
        "If used this flag with --workspace, --each-feature, or --feature-powerset, artifacts will be removed before each run.",
        "Note that dependencies artifacts will be preserved.",
    ]),
    ("-v", "--verbose", "Use verbose output", &[]),
    ("", "--color <WHEN>", "Coloring: auto, always, never", &[
        "This flag will be propagated to cargo.",
    ]),
    ("-h", "--help", "Prints help information", &[]),
    ("-V", "--version", "Prints version information", &[]),
];

struct Help {
    long: bool,
    term_size: usize,
}

impl Help {
    fn long() -> Self {
        Self { long: true, term_size: term_size::dimensions().map_or(120, |(w, _)| w) }
    }

    fn short() -> Self {
        Self { long: false, term_size: term_size::dimensions().map_or(120, |(w, _)| w) }
    }
}

impl fmt::Display for Help {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn write(
            f: &mut fmt::Formatter<'_>,
            indent: usize,
            require_first_indent: bool,
            term_size: usize,
            desc: &str,
        ) -> fmt::Result {
            if require_first_indent {
                (0..indent).try_for_each(|_| write!(f, " "))?;
            }
            let mut written = 0;
            let size = term_size - indent;
            for s in desc.split(' ') {
                if written + s.len() + 1 >= size {
                    writeln!(f)?;
                    (0..indent).try_for_each(|_| write!(f, " "))?;
                    write!(f, "{}", s)?;
                    written = s.len();
                } else if written == 0 {
                    write!(f, "{}", s)?;
                    written += s.len();
                } else {
                    write!(f, " {}", s)?;
                    written += s.len() + 1;
                }
            }
            Ok(())
        }

        writeln!(
            f,
            "\
{0} {1}\n{2}
USAGE:
    cargo hack [OPTIONS] [SUBCOMMAND]\n
Use -h for short descriptions and --help for more details.\n
OPTIONS:",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_DESCRIPTION")
        )?;

        for &(short, long, desc, additional) in HELP {
            write!(f, "    {:2}{} ", short, if short.is_empty() { " " } else { "," })?;
            if self.long {
                writeln!(f, "{}", long)?;
                write(f, 12, true, self.term_size, desc)?;
                writeln!(f, ".\n")?;
                for desc in additional {
                    write(f, 12, true, self.term_size, desc)?;
                    writeln!(f, "\n")?;
                }
            } else {
                write!(f, "{:32} ", long)?;
                write(f, 41, false, self.term_size, desc)?;
                writeln!(f)?;
            }
        }
        if !self.long {
            writeln!(f)?;
        }

        writeln!(
            f,
            "\
Some common cargo commands are (see all commands with --list):
        build       Compile the current package
        check       Analyze the current package and report errors, but don't build object files
        run         Run a binary or example of the local package
        test        Run the tests"
        )
    }
}

pub(crate) struct Args<'a> {
    pub(crate) leading_args: Vec<&'a str>,
    pub(crate) trailing_args: &'a [String],

    pub(crate) subcommand: Option<&'a str>,

    /// --manifest-path <PATH>
    pub(crate) manifest_path: Option<&'a str>,
    /// -p, --package <SPEC>...
    pub(crate) package: Vec<&'a str>,
    /// --exclude <SPEC>...
    pub(crate) exclude: Vec<&'a str>,
    /// --workspace, (--all)
    pub(crate) workspace: bool,
    /// --each-feature
    pub(crate) each_feature: bool,
    /// --feature-powerset
    pub(crate) feature_powerset: bool,
    /// --no-dev-deps
    pub(crate) no_dev_deps: bool,
    /// --remove-dev-deps
    pub(crate) remove_dev_deps: bool,
    /// --ignore-private
    pub(crate) ignore_private: bool,
    /// --ignore-unknown-features
    pub(crate) ignore_unknown_features: bool,
    /// --optional-deps [DEPS]...
    pub(crate) optional_deps: Option<Vec<&'a str>>,
    /// --clean-per-run
    pub(crate) clean_per_run: bool,
    /// --depth <NUM>
    pub(crate) depth: Option<usize>,
    /// --include-features
    pub(crate) include_features: Vec<&'a str>,

    /// --no-default-features
    pub(crate) no_default_features: bool,
    /// -v, --verbose, -vv
    pub(crate) verbose: bool,

    // Note: These values are not always exactly the same as the input.
    // Error messages should not assume that these options have been specified.
    /// --exclude-features <FEATURES>..., --skip <FEATURES>...
    pub(crate) exclude_features: Vec<&'a str>,
    /// --exclude-no-default-features, (--skip-no-default-features)
    pub(crate) exclude_no_default_features: bool,
    /// --exclude-all-features
    pub(crate) exclude_all_features: bool,

    // flags that will be propagated to cargo
    /// --features <FEATURES>...
    pub(crate) features: Vec<&'a str>,
    /// --color <WHEN>
    pub(crate) color: Option<Coloring>,
}

#[derive(Clone, Copy)]
pub(crate) enum Coloring {
    Auto,
    Always,
    Never,
}

impl Coloring {
    pub(crate) fn color_choice(color: Option<Self>) -> ColorChoice {
        match color {
            Some(Coloring::Auto) | None => ColorChoice::Auto,
            Some(Coloring::Always) => ColorChoice::Always,
            Some(Coloring::Never) => ColorChoice::Never,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Coloring::Auto => "auto",
            Coloring::Always => "always",
            Coloring::Never => "never",
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

pub(crate) fn raw() -> RawArgs {
    let mut args = env::args();
    let _ = args.next(); // executable name
    RawArgs(args.collect())
}

pub(crate) struct RawArgs(Vec<String>);

pub(crate) fn perse_args<'a>(
    raw: &'a RawArgs,
    coloring: &mut Option<Coloring>,
) -> Result<Args<'a>> {
    let mut iter = raw.0.iter();
    let mut args = iter.by_ref().map(String::as_str).peekable();
    match args.next() {
        Some(a) if a == "hack" => {}
        Some(_) | None => {
            println!("{}", Help::short());
            std::process::exit(0);
        }
    }

    let mut leading = Vec::new();
    let mut subcommand: Option<&'a str> = None;

    let mut manifest_path = None;
    let mut color = None;

    let mut package = Vec::new();
    let mut exclude = Vec::new();
    let mut features = Vec::new();
    let mut optional_deps = None;
    let mut include_features = Vec::new();

    let mut workspace = None;
    let mut no_dev_deps = false;
    let mut remove_dev_deps = false;
    let mut each_feature = false;
    let mut feature_powerset = false;
    let mut ignore_private = false;
    let mut ignore_unknown_features = false;
    let mut clean_per_run = false;
    let mut depth = None;

    let mut exclude_features = Vec::new();
    let mut exclude_no_default_features = false;
    let mut exclude_all_features = false;
    let mut skip_no_default_features = false;

    let mut verbose = false;
    let mut no_default_features = false;
    let mut all_features = false;

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
                subcommand.get_or_insert(arg);
                leading.push(arg);
                continue;
            }

            macro_rules! parse_opt {
                ($opt:ident, $pat:expr, $help:expr) => {
                    if arg == $pat {
                        if $opt.is_some() {
                            return Err(multi_arg($help, subcommand));
                        }
                        let next = args.next().ok_or_else(|| req_arg($help, subcommand))?;
                        $opt = Some(next);
                        continue;
                    } else if arg.starts_with(concat!($pat, "=")) {
                        if $opt.is_some() {
                            return Err(multi_arg($help, subcommand));
                        }
                        let next =
                            arg.splitn(2, '=').nth(1).ok_or_else(|| req_arg($help, subcommand))?;
                        $opt = Some(next);
                        continue;
                    }
                };
            }

            macro_rules! parse_multi_opt {
                ($v:ident, $allow_split:expr, $require_value:expr, $pat:expr, $help:expr) => {
                    if arg == $pat {
                        if !$require_value && args.peek().map_or(true, |s| s.starts_with('-')) {
                            continue;
                        }
                        let arg = args.next().ok_or_else(|| req_arg($help, subcommand))?;
                        if $allow_split {
                            if arg.contains(',') {
                                $v.extend(arg.split(','));
                            } else {
                                $v.extend(arg.split(' '));
                            }
                        } else {
                            $v.push(arg);
                        }
                        continue;
                    } else if arg.starts_with(concat!($pat, "=")) {
                        let mut arg =
                            arg.splitn(2, '=').nth(1).ok_or_else(|| req_arg($help, subcommand))?;
                        if $allow_split {
                            if arg.starts_with('\'') && arg.ends_with('\'')
                                || arg.starts_with('"') && arg.ends_with('"')
                            {
                                arg = &arg[1..arg.len() - 1];
                            }
                            if arg.contains(',') {
                                $v.extend(arg.split(','));
                            } else {
                                $v.extend(arg.split(' '));
                            }
                        } else {
                            $v.push(arg);
                        }
                        continue;
                    }
                };
            }

            macro_rules! parse_flag {
                ($flag:ident) => {
                    if mem::replace(&mut $flag, true) {
                        return Err(multi_arg(&arg, subcommand));
                    } else {
                        continue;
                    }
                };
            }

            parse_opt!(manifest_path, "--manifest-path", "--manifest-path <PATH>");
            parse_opt!(depth, "--depth", "--depth <NUM>");
            parse_opt!(color, "--color", "--color <WHEN>");

            parse_multi_opt!(package, false, true, "--package", "--package <SPEC>...");
            parse_multi_opt!(package, false, true, "-p", "--package <SPEC>...");
            parse_multi_opt!(exclude, false, true, "--exclude", "--exclude <SPEC>...");
            parse_multi_opt!(features, true, true, "--features", "--features <FEATURES>...");
            parse_multi_opt!(exclude_features, true, true, "--skip", "--skip <FEATURES>...");
            parse_multi_opt!(
                exclude_features,
                true,
                true,
                "--exclude-features",
                "--exclude-features <FEATURES>..."
            );
            parse_multi_opt!(
                include_features,
                true,
                true,
                "--include-features",
                "--include-features <FEATURES>..."
            );

            if arg.starts_with("--optional-deps") {
                if optional_deps.is_some() {
                    return Err(multi_arg(arg, subcommand));
                }
                let optional_deps = optional_deps.get_or_insert_with(Vec::new);
                parse_multi_opt!(
                    optional_deps,
                    true,
                    false,
                    "--optional-deps",
                    "--optional-deps [DEPS]..."
                );
            }

            match &*arg {
                "--workspace" | "--all" => {
                    if let Some(arg) = workspace.replace(arg) {
                        return Err(multi_arg(arg, subcommand));
                    }
                    continue;
                }
                "--no-dev-deps" => parse_flag!(no_dev_deps),
                "--remove-dev-deps" => parse_flag!(remove_dev_deps),
                "--each-feature" => parse_flag!(each_feature),
                "--feature-powerset" => parse_flag!(feature_powerset),
                "--ignore-private" => parse_flag!(ignore_private),
                "--exclude-no-default-features" => {
                    if exclude_no_default_features || skip_no_default_features {
                        return Err(multi_arg(arg, subcommand));
                    }
                    exclude_no_default_features = true;
                    continue;
                }
                "--skip-no-default-features" => {
                    if exclude_no_default_features || skip_no_default_features {
                        return Err(multi_arg("--exclude-no-default-features", subcommand));
                    }
                    skip_no_default_features = true;
                    continue;
                }
                "--exclude-all-features" => parse_flag!(exclude_all_features),
                "--clean-per-run" => parse_flag!(clean_per_run),
                "--ignore-unknown-features" => parse_flag!(ignore_unknown_features),
                "--ignore-non-exist-features" => bail!(
                    "--ignore-non-exist-features was removed, use --ignore-unknown-features instead"
                ),
                // allow multiple uses
                "--verbose" | "-v" | "-vv" => {
                    verbose = true;
                    continue;
                }
                "--no-default-features" => no_default_features = true,
                "--all-features" => all_features = true,
                _ => {}
            }

            leading.push(arg);
        }

        Ok(())
    })();

    let color = color.map(str::parse).transpose()?;
    *coloring = color;

    res?;

    if leading.is_empty() && !remove_dev_deps || subcommand.is_none() && leading.contains(&"-h") {
        println!("{}", Help::short());
        std::process::exit(0);
    } else if subcommand.is_none() && leading.contains(&"--help") {
        println!("{}", Help::long());
        std::process::exit(0);
    } else if leading.iter().any(|&a| a == "--version" || a == "-V" || a == "-vV" || a == "-Vv") {
        print_version();
        std::process::exit(0);
    }

    if !exclude.is_empty() && workspace.is_none() {
        // TODO: This is the same behavior as cargo, but should we allow it to be used
        // in the root of a virtual workspace as well?
        bail!("--exclude can only be used together with --workspace");
    }
    if ignore_unknown_features && features.is_empty() && include_features.is_empty() {
        bail!(
            "--ignore-unknown-features can only be used together with either --features or --include-features"
        );
    }
    if !each_feature && !feature_powerset {
        if optional_deps.is_some() {
            bail!(
                "--optional-deps can only be used together with either --each-feature or --feature-powerset"
            );
        } else if !exclude_features.is_empty() {
            bail!(
                "--exclude-features (--skip) can only be used together with either --each-feature or --feature-powerset"
            );
        } else if exclude_no_default_features || skip_no_default_features {
            bail!(
                "--exclude-no-default-features can only be used together with either --each-feature or --feature-powerset"
            );
        } else if exclude_all_features {
            bail!(
                "--exclude-all-features can only be used together with either --each-feature or --feature-powerset"
            );
        } else if !include_features.is_empty() {
            bail!(
                "--include-features can only be used together with either --each-feature or --feature-powerset"
            );
        }
    }
    if depth.is_some() && !feature_powerset {
        bail!("--depth can only be used together with --feature-powerset");
    }
    let depth = depth.map(str::parse::<usize>).transpose()?;

    if let Some(subcommand) = subcommand {
        if subcommand == "test" || subcommand == "bench" {
            if remove_dev_deps {
                bail!("--remove-dev-deps may not be used together with {} subcommand", subcommand);
            } else if no_dev_deps {
                bail!("--no-dev-deps may not be used together with {} subcommand", subcommand);
            }
        }
    }
    if let Some(pos) = leading.iter().position(|a| match &**a {
        "--example" | "--examples" | "--test" | "--tests" | "--bench" | "--benches"
        | "--all-targets" => true,
        _ => a.starts_with("--example=") || a.starts_with("--test=") || a.starts_with("--bench="),
    }) {
        if remove_dev_deps {
            bail!("--remove-dev-deps may not be used together with {}", leading[pos]);
        } else if no_dev_deps {
            bail!("--no-dev-deps may not be used together with {}", leading[pos]);
        }
    }

    if no_dev_deps && remove_dev_deps {
        bail!("--no-dev-deps may not be used together with --remove-dev-deps");
    }
    if each_feature && feature_powerset {
        bail!("--each-feature may not be used together with --feature-powerset");
    }
    if all_features && each_feature {
        bail!("--all-features may not be used together with --each-feature");
    }
    if all_features && feature_powerset {
        bail!("--all-features may not be used together with --feature-powerset");
    }

    if subcommand.is_none() {
        if leading.contains(&"--list") {
            let mut line = ProcessBuilder::new(crate::cargo_binary(), verbose);
            line.arg("--list");
            line.exec()?;
            std::process::exit(0);
        } else if !remove_dev_deps {
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

    if skip_no_default_features {
        warn!(
            color,
            "--skip-no-default-features is deprecated, use --exclude-no-default-features flag instead"
        );
        exclude_no_default_features = true;
    }
    if no_dev_deps {
        info!(
            color,
            "--no-dev-deps removes dev-dependencies from real `Cargo.toml` while cargo-hack is running and restores it when finished"
        )
    }

    exclude_no_default_features |= no_default_features;
    exclude_features.extend_from_slice(&features);

    Ok(Args {
        leading_args: leading,
        trailing_args: iter.as_slice(),

        subcommand,

        manifest_path,
        package,
        exclude,
        workspace: workspace.is_some(),
        each_feature,
        feature_powerset,
        no_dev_deps,
        remove_dev_deps,
        ignore_private,
        ignore_unknown_features,
        optional_deps,
        clean_per_run,
        depth,
        include_features,

        no_default_features,
        verbose,

        exclude_features,
        exclude_no_default_features,
        exclude_all_features,

        features,
        color,
    })
}

fn req_arg(arg: &str, subcommand: Option<&str>) -> Error {
    format_err!(
        "\
The argument '{0}' requires a value but none was supplied

USAGE:
    cargo hack{1} {0}

For more information try --help
",
        arg,
        subcommand.map_or_else(String::new, |subcommand| String::from(" ") + subcommand)
    )
}

fn multi_arg(arg: &str, subcommand: Option<&str>) -> Error {
    format_err!(
        "\
The argument '{0}' was provided more than once, but cannot be used multiple times

USAGE:
    cargo hack{1} {0}

For more information try --help
",
        arg,
        subcommand.map_or_else(String::new, |subcommand| String::from(" ") + subcommand)
    )
}

#[cfg(test)]
mod tests {
    use super::Help;
    use crate::Result;
    use std::{env, fs, path::PathBuf};

    fn assert_eq(expected_path: &str, actual: &str) {
        (|| -> Result<()> {
            let manifest_dir = env::var_os("CARGO_MANIFEST_DIR")
                .map(PathBuf::from)
                .expect("CARGO_MANIFEST_DIR not set");
            let expected_path = manifest_dir.join(expected_path);
            let expected = fs::read_to_string(&expected_path)?;
            if expected != actual {
                if env::var_os("CI").map_or(false, |v| v == "true") {
                    panic!(
                        "assertion failed:\n\nEXPECTED:\n{0}\n{1}\n{0}\n\nACTUAL:\n{0}\n{2}\n{0}\n",
                        "-".repeat(60),
                        expected,
                        actual,
                    );
                } else {
                    fs::write(&expected_path, actual)?;
                }
            }
            Ok(())
        })()
        .unwrap_or_else(|e| panic!("{:#}", e))
    }

    #[test]
    fn long_help() {
        let actual = &Help { long: true, term_size: 200 }.to_string();
        assert_eq("tests/long-help.txt", actual);
    }

    #[test]
    fn short_help() {
        let actual = &Help { long: false, term_size: 200 }.to_string();
        assert_eq("tests/short-help.txt", actual);
    }
}
