use anyhow::{bail, format_err, Error};
use std::{env, fmt, mem, str::FromStr};
use termcolor::ColorChoice;

use crate::{ProcessBuilder, Result};

fn print_version() {
    println!("{0} {1}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"),)
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
    ("", "--skip <FEATURES>...", "Space-separated list of features to skip", &[
        "To skip run of default feature, using value `--skip default`.",
        "This flag can only be used together with either --each-feature flag or --feature-powerset flag.",
    ]),
    ("", "--skip-no-default-features", "Skip run of just --no-default-features flag", &[
        "This flag can only be used together with either --each-feature flag or --feature-powerset flag.",
    ]),
    ("", "--skip-all-features", "Skip run of just --all-features flag", &[
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
        &[
            "This flag can only be used in the root of a virtual workspace or together with --workspace.",
        ],
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
    fn new(long: bool) -> Self {
        Self { long, term_size: term_size::dimensions().map_or(120, |(w, _)| w) }
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
                write!(f, "{:26} ", long)?;
                write(f, 35, false, self.term_size, desc)?;
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

pub(crate) struct Args {
    pub(crate) leading_args: Vec<String>,
    pub(crate) trailing_args: Vec<String>,

    pub(crate) subcommand: Option<String>,

    /// --manifest-path <PATH>
    pub(crate) manifest_path: Option<String>,
    /// -p, --package <SPEC>...
    pub(crate) package: Vec<String>,
    /// --exclude <SPEC>...
    pub(crate) exclude: Vec<String>,
    /// --workspace, (--all)
    pub(crate) workspace: bool,
    /// --each-feature
    pub(crate) each_feature: bool,
    /// --feature-powerset
    pub(crate) feature_powerset: bool,
    /// --skip <FEATURES>...
    pub(crate) skip: Vec<String>,
    /// --no-dev-deps
    pub(crate) no_dev_deps: bool,
    /// --remove-dev-deps
    pub(crate) remove_dev_deps: bool,
    /// --ignore-private
    pub(crate) ignore_private: bool,
    /// --ignore-unknown-features
    pub(crate) ignore_unknown_features: bool,
    /// --optional-deps [DEPS]...
    pub(crate) optional_deps: Option<Vec<String>>,
    /// --skip-no-default-features
    pub(crate) skip_no_default_features: bool,
    /// --skip-all-features
    pub(crate) skip_all_features: bool,
    /// --clean-per-run
    pub(crate) clean_per_run: bool,
    /// -v, --verbose, -vv
    pub(crate) verbose: bool,
    /// --depth <NUM>
    pub(crate) depth: Option<usize>,

    // flags that will be propagated to cargo
    /// --features <FEATURES>...
    pub(crate) features: Vec<String>,
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

pub(crate) fn args(coloring: &mut Option<Coloring>) -> Result<Option<Args>> {
    let mut args = env::args().peekable();
    let _ = args.next(); // executable name
    match &args.next() {
        Some(a) if a == "hack" => {}
        Some(_) | None => {
            println!("{}", Help::new(false));
            return Ok(None);
        }
    }

    let mut leading = Vec::new();
    let mut subcommand: Option<String> = None;

    let mut manifest_path = None;
    let mut color = None;

    let mut package = Vec::new();
    let mut exclude = Vec::new();
    let mut features = Vec::new();
    let mut skip = Vec::new();
    let mut optional_deps = None;

    let mut workspace = None;
    let mut no_dev_deps = false;
    let mut remove_dev_deps = false;
    let mut each_feature = false;
    let mut feature_powerset = false;
    let mut ignore_private = false;
    let mut ignore_unknown_features = false;
    let mut skip_no_default_features = false;
    let mut skip_all_features = false;
    let mut clean_per_run = false;
    let mut verbose = false;
    let mut depth = None;

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
                subcommand.get_or_insert_with(|| arg.clone());
                leading.push(arg);
                continue;
            }

            macro_rules! parse_opt {
                ($opt:ident, $propagate:expr, $pat:expr, $help:expr) => {
                    if arg == $pat {
                        if $opt.is_some() {
                            return Err(multi_arg($help, subcommand.as_ref()));
                        }
                        let next =
                            args.next().ok_or_else(|| req_arg($help, subcommand.as_ref()))?;
                        if $propagate {
                            $opt = Some(next.clone());
                            leading.push(arg);
                            leading.push(next);
                        } else {
                            $opt = Some(next);
                        }
                        continue;
                    } else if arg.starts_with(concat!($pat, "=")) {
                        if $opt.is_some() {
                            return Err(multi_arg($help, subcommand.as_ref()));
                        }
                        let next = arg
                            .splitn(2, '=')
                            .nth(1)
                            .ok_or_else(|| req_arg($help, subcommand.as_ref()))?;
                        $opt = Some(next.to_string());
                        if $propagate {
                            leading.push(arg);
                        }
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
                        let arg = args.next().ok_or_else(|| req_arg($help, subcommand.as_ref()))?;
                        if $allow_split {
                            if arg.contains(',') {
                                $v.extend(arg.split(',').map(ToString::to_string));
                            } else {
                                $v.extend(arg.split(' ').map(ToString::to_string));
                            }
                        } else {
                            $v.push(arg);
                        }
                        continue;
                    } else if arg.starts_with(concat!($pat, "=")) {
                        let mut arg = arg
                            .splitn(2, '=')
                            .nth(1)
                            .ok_or_else(|| req_arg($help, subcommand.as_ref()))?;
                        if $allow_split {
                            if arg.starts_with('\'') && arg.ends_with('\'')
                                || arg.starts_with('"') && arg.ends_with('"')
                            {
                                arg = &arg[1..arg.len() - 1];
                            }
                            if arg.contains(',') {
                                $v.extend(arg.split(',').map(ToString::to_string));
                            } else {
                                $v.extend(arg.split(' ').map(ToString::to_string));
                            }
                        } else {
                            $v.push(arg.to_string());
                        }
                        continue;
                    }
                };
            }

            macro_rules! parse_flag {
                ($flag:ident) => {
                    if mem::replace(&mut $flag, true) {
                        return Err(multi_arg(&arg, subcommand.as_ref()));
                    }
                };
            }

            parse_opt!(manifest_path, false, "--manifest-path", "--manifest-path <PATH>");
            parse_opt!(depth, false, "--depth", "--depth <NUM>");
            parse_opt!(color, true, "--color", "--color <WHEN>");

            parse_multi_opt!(package, false, true, "--package", "--package <SPEC>...");
            parse_multi_opt!(package, false, true, "-p", "--package <SPEC>...");
            parse_multi_opt!(exclude, false, true, "--exclude", "--exclude <SPEC>...");
            parse_multi_opt!(features, true, true, "--features", "--features <FEATURES>...");
            parse_multi_opt!(skip, true, true, "--skip", "--skip <FEATURES>...");

            if arg.starts_with("--optional-deps") {
                if optional_deps.is_some() {
                    return Err(multi_arg(&arg, subcommand.as_ref()));
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
                        return Err(multi_arg(&arg, subcommand.as_ref()));
                    }
                }
                "--no-dev-deps" => parse_flag!(no_dev_deps),
                "--remove-dev-deps" => parse_flag!(remove_dev_deps),
                "--each-feature" => parse_flag!(each_feature),
                "--feature-powerset" => parse_flag!(feature_powerset),
                "--ignore-private" => parse_flag!(ignore_private),
                "--skip-no-default-features" => parse_flag!(skip_no_default_features),
                "--skip-all-features" => parse_flag!(skip_all_features),
                "--clean-per-run" => parse_flag!(clean_per_run),
                "--ignore-unknown-features" => parse_flag!(ignore_unknown_features),
                "--ignore-non-exist-features" => bail!(
                    "--ignore-non-exist-features was removed, use --ignore-unknown-features instead"
                ),
                // allow multiple uses
                "--verbose" | "-v" | "-vv" => verbose = true,
                _ => leading.push(arg),
            }
        }

        Ok(())
    })();

    let color = color.map(|c| c.parse()).transpose()?;
    *coloring = color;

    res?;

    if leading.is_empty() && !remove_dev_deps
        || subcommand.is_none() && leading.iter().any(|a| a == "-h")
    {
        println!("{}", Help::new(false));
        return Ok(None);
    } else if subcommand.is_none() && leading.iter().any(|a| a == "--help") {
        println!("{}", Help::new(true));
        return Ok(None);
    } else if leading.iter().any(|a| a == "--version" || a == "-V" || a == "-vV" || a == "-Vv") {
        print_version();
        return Ok(None);
    }

    if !exclude.is_empty() && workspace.is_none() {
        // TODO: This is the same behavior as cargo, but should we allow it to be used
        // in the root of a virtual workspace as well?
        bail!("--exclude can only be used together with --workspace");
    }
    if ignore_unknown_features && features.is_empty() {
        // TODO: Once https://github.com/taiki-e/cargo-hack/issues/52 implemented,
        // allow --include-features.
        bail!("--ignore-unknown-features can only be used together with --features");
    }
    if !each_feature && !feature_powerset {
        if optional_deps.is_some() {
            bail!(
                "--optional-deps can only be used together with either --each-feature or --feature-powerset"
            );
        } else if !skip.is_empty() {
            bail!(
                "--skip can only be used together with either --each-feature or --feature-powerset"
            );
        } else if skip_no_default_features {
            bail!(
                "--skip-no-default-features can only be used together with either --each-feature or --feature-powerset"
            );
        } else if skip_all_features {
            bail!(
                "--skip-all-features can only be used together with either --each-feature or --feature-powerset"
            );
        }
    }
    if depth.is_some() && !feature_powerset {
        bail!("--depth can only be used together with --feature-powerset");
    }
    let depth = depth.map(|s| s.parse::<usize>()).transpose()?;

    if let Some(subcommand) = &subcommand {
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

    if subcommand.is_none() {
        if leading.iter().any(|a| a == "--list") {
            let mut line = ProcessBuilder::new(crate::cargo_binary(), verbose);
            line.arg("--list");
            line.exec()?;
            return Ok(None);
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

    if no_dev_deps {
        info!(
            color,
            "--no-dev-deps removes dev-dependencies from real `Cargo.toml` while cargo-hack is running and restores it when finished"
        )
    }

    Ok(Some(Args {
        leading_args: leading,
        trailing_args: args.collect::<Vec<_>>(),

        subcommand,

        manifest_path,
        package,
        exclude,
        workspace: workspace.is_some(),
        each_feature,
        feature_powerset,
        skip,
        no_dev_deps,
        remove_dev_deps,
        ignore_private,
        ignore_unknown_features,
        optional_deps,
        skip_no_default_features,
        skip_all_features,
        clean_per_run,
        depth,

        features,
        color,
        verbose,
    }))
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
