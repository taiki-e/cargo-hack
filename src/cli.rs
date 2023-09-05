use std::{
    collections::BTreeSet,
    env,
    ffi::{OsStr, OsString},
    fmt, mem,
};

use anyhow::{bail, format_err, Result};
use lexopt::{
    Arg::{Long, Short, Value},
    ValueExt,
};

use crate::{term, Feature, Rustup};

pub(crate) struct Args {
    pub(crate) leading_args: Vec<String>,
    pub(crate) trailing_args: Vec<String>,

    pub(crate) subcommand: Option<String>,

    /// --manifest-path <PATH>
    pub(crate) manifest_path: Option<String>,
    /// --no-manifest-path
    pub(crate) no_manifest_path: bool,
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
    /// --no-dev-deps
    pub(crate) no_dev_deps: bool,
    /// --remove-dev-deps
    pub(crate) remove_dev_deps: bool,
    /// --no-private
    pub(crate) no_private: bool,
    /// --ignore-private
    pub(crate) ignore_private: bool,
    /// --ignore-unknown-features
    pub(crate) ignore_unknown_features: bool,
    /// --clean-per-run
    pub(crate) clean_per_run: bool,
    /// --clean-per-version
    pub(crate) clean_per_version: bool,
    /// --keep-going
    pub(crate) keep_going: bool,
    /// --print-command-list
    pub(crate) print_command_list: bool,
    /// --version-range
    pub(crate) version_range: Option<String>,
    /// --version-step
    pub(crate) version_step: Option<String>,

    // options for --each-feature and --feature-powerset
    /// --optional-deps [DEPS]...
    pub(crate) optional_deps: Option<Vec<String>>,
    /// --include-features <FEATURES>...
    pub(crate) include_features: Vec<Feature>,
    /// --include-deps-features
    pub(crate) include_deps_features: bool,

    // Note: These values are not always exactly the same as the input.
    // Error messages should not assume that these options have been specified.
    /// --exclude-features <FEATURES>..., --skip <FEATURES>...
    pub(crate) exclude_features: Vec<String>,
    /// --exclude-no-default-features
    pub(crate) exclude_no_default_features: bool,
    /// --exclude-all-features
    pub(crate) exclude_all_features: bool,

    // options for --feature-powerset
    /// --depth <NUM>
    pub(crate) depth: Option<usize>,
    /// --group-features <FEATURES>...
    pub(crate) group_features: Vec<Feature>,
    /// --at-least-one-of <FEATURES>...
    /// Implies --exclude-no-default-features. Can be specified multiple times.
    pub(crate) at_least_one_of: Vec<Feature>,

    // options that will be propagated to cargo
    /// --features <FEATURES>...
    pub(crate) features: Vec<String>,
    /// --target <TRIPLE>...
    pub(crate) target: Vec<String>,

    // propagated to cargo (as a part of leading_args)
    /// --no-default-features
    pub(crate) no_default_features: bool,
}

impl Args {
    pub(crate) fn parse(cargo: &OsStr) -> Result<Self> {
        const SUBCMD: &str = "hack";

        // rustc/cargo args must be valid Unicode
        // https://github.com/rust-lang/rust/blob/1.70.0/compiler/rustc_driver_impl/src/lib.rs#L1366-L1376
        fn handle_args(
            args: impl IntoIterator<Item = impl Into<OsString>>,
        ) -> impl Iterator<Item = Result<String>> {
            args.into_iter().enumerate().map(|(i, arg)| {
                arg.into()
                    .into_string()
                    .map_err(|arg| format_err!("argument {} is not valid Unicode: {arg:?}", i + 1))
            })
        }

        let mut raw_args = handle_args(env::args_os());
        raw_args.next(); // cargo
        match raw_args.next().transpose()? {
            Some(a) if a == SUBCMD => {}
            Some(a) => bail!("expected subcommand '{SUBCMD}', found argument '{a}'"),
            None => bail!("expected subcommand '{SUBCMD}'"),
        }
        let mut args = vec![];
        for arg in &mut raw_args {
            let arg = arg?;
            if arg == "--" {
                break;
            }
            args.push(arg);
        }
        let rest = raw_args.collect::<Result<Vec<_>>>()?;

        let mut cargo_args = vec![];
        let mut subcommand: Option<String> = None;

        let mut manifest_path = None;
        let mut color = None;

        let mut package = vec![];
        let mut exclude = vec![];
        let mut features = vec![];

        let mut workspace = false;
        let mut no_dev_deps = false;
        let mut remove_dev_deps = false;
        let mut each_feature = false;
        let mut feature_powerset = false;
        let mut no_private = false;
        let mut ignore_private = false;
        let mut ignore_unknown_features = false;
        let mut clean_per_run = false;
        let mut clean_per_version = false;
        let mut keep_going = false;
        let mut print_command_list = false;
        let mut no_manifest_path = false;
        let mut version_range = None;
        let mut version_step = None;

        let mut optional_deps = None;
        let mut include_features = vec![];
        let mut at_least_one_of = vec![];
        let mut include_deps_features = false;

        let mut exclude_features = vec![];
        let mut exclude_no_default_features = false;
        let mut exclude_all_features = false;

        let mut group_features: Vec<String> = vec![];
        let mut depth = None;

        let mut verbose = 0;
        let mut no_default_features = false;
        let mut all_features = false;

        // Cargo seems to be deduplicating targets internally using BTreeSet or BTreeMap.
        // For example, the following commands all run the test once each in the order aarch64 -> x86_64.
        //
        // ```
        // cargo test -v --target x86_64-apple-darwin --target aarch64-apple-darwin
        // cargo test -v --target aarch64-apple-darwin --target x86_64-apple-darwin
        // cargo test -v --target x86_64-apple-darwin --target aarch64-apple-darwin --target x86_64-apple-darwin
        // ```
        let mut target = BTreeSet::new();

        let mut parser = lexopt::Parser::from_args(args);
        let mut next_flag: Option<OwnedFlag> = None;
        loop {
            let arg = next_flag.take();
            let arg = match &arg {
                Some(flag) => flag.as_arg(),
                None => match parser.next()? {
                    Some(arg) => arg,
                    None => break,
                },
            };

            macro_rules! parse_opt {
                ($opt:ident, $propagate:expr $(,)?) => {{
                    if $opt.is_some() {
                        multi_arg(&arg, subcommand.as_deref())?;
                    }
                    if $propagate {
                        cargo_args.push(format_flag(&arg));
                    }
                    let val = parser.value()?.string()?;
                    if $propagate {
                        cargo_args.push(val.clone());
                    }
                    $opt = Some(val);
                }};
            }

            macro_rules! parse_multi_opt {
                ($v:ident $(,)?) => {{
                    let val = parser.value()?;
                    let mut val = val.to_str().unwrap();
                    if val.starts_with('\'') && val.ends_with('\'')
                        || val.starts_with('"') && val.ends_with('"')
                    {
                        val = &val[1..val.len() - 1];
                    }
                    let sep = if val.contains(',') { ',' } else { ' ' };
                    $v.extend(val.split(sep).filter(|s| !s.is_empty()).map(str::to_owned));
                }};
            }

            macro_rules! parse_flag {
                ($flag:ident $(,)?) => {
                    if mem::replace(&mut $flag, true) {
                        multi_arg(&arg, subcommand.as_deref())?;
                    }
                };
            }

            match arg {
                Long("color") => parse_opt!(color, true),
                Long("target") => {
                    target.insert(parser.value()?.parse()?);
                }

                Long("manifest-path") => parse_opt!(manifest_path, false),
                Long("depth") => parse_opt!(depth, false),
                Long("version-range") => parse_opt!(version_range, false),
                Long("version-step") => parse_opt!(version_step, false),

                Short('p') | Long("package") => package.push(parser.value()?.parse()?),
                Long("exclude") => exclude.push(parser.value()?.parse()?),
                Long("group-features") => group_features.push(parser.value()?.parse()?),

                Short('F') | Long("features") => parse_multi_opt!(features),
                Long("skip" | "exclude-features") => parse_multi_opt!(exclude_features),
                Long("include-features") => parse_multi_opt!(include_features),

                Long("optional-deps") => {
                    if optional_deps.is_some() {
                        multi_arg(&arg, subcommand.as_deref())?;
                    }
                    let optional_deps = optional_deps.get_or_insert_with(Vec::new);
                    let val = match parser.optional_value() {
                        Some(val) => val,
                        None => match parser.next()? {
                            Some(Value(val)) => val,
                            Some(arg) => {
                                next_flag = Some(arg.into());
                                continue;
                            }
                            None => break,
                        },
                    };
                    let mut val = val.to_str().unwrap();
                    if val.starts_with('\'') && val.ends_with('\'')
                        || val.starts_with('"') && val.ends_with('"')
                    {
                        val = &val[1..val.len() - 1];
                    }
                    if val.contains(',') {
                        optional_deps.extend(val.split(',').map(str::to_owned));
                    } else {
                        optional_deps.extend(val.split(' ').map(str::to_owned));
                    }
                }

                Long("workspace" | "all") => parse_flag!(workspace),
                Long("no-dev-deps") => parse_flag!(no_dev_deps),
                Long("remove-dev-deps") => parse_flag!(remove_dev_deps),
                Long("each-feature") => parse_flag!(each_feature),
                Long("feature-powerset") => parse_flag!(feature_powerset),
                Long("at-least-one-of") => at_least_one_of.push(parser.value()?.parse()?),
                Long("no-private") => parse_flag!(no_private),
                Long("ignore-private") => parse_flag!(ignore_private),
                Long("exclude-no-default-features") => parse_flag!(exclude_no_default_features),
                Long("exclude-all-features") => parse_flag!(exclude_all_features),
                Long("include-deps-features") => parse_flag!(include_deps_features),
                Long("clean-per-run") => parse_flag!(clean_per_run),
                Long("clean-per-version") => parse_flag!(clean_per_version),
                Long("keep-going") => parse_flag!(keep_going),
                Long("print-command-list") => parse_flag!(print_command_list),
                Long("no-manifest-path") => parse_flag!(no_manifest_path),
                Long("ignore-unknown-features") => parse_flag!(ignore_unknown_features),
                Short('v') | Long("verbose") => verbose += 1,

                // propagated
                Long("no-default-features") => {
                    no_default_features = true;
                    cargo_args.push("--no-default-features".to_owned());
                }
                Long("all-features") => {
                    all_features = true;
                    cargo_args.push("--all-features".to_owned());
                }

                Short('h') if subcommand.is_none() => {
                    println!("{}", Help::short());
                    std::process::exit(0);
                }
                Long("help") if subcommand.is_none() => {
                    println!("{}", Help::long());
                    std::process::exit(0);
                }
                Short('V') | Long("version") if subcommand.is_none() => {
                    println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }

                // passthrough
                Long(flag) => {
                    removed_flags(flag)?;
                    similar_flags(flag, subcommand.as_deref())?;
                    let flag = format!("--{flag}");
                    if let Some(val) = parser.optional_value() {
                        cargo_args.push(format!("{flag}={}", val.string()?));
                    } else {
                        cargo_args.push(flag);
                    }
                }
                Short(flag) => {
                    if matches!(flag, 'q' | 'r') {
                        // To handle combined short flags properly, handle known
                        // short flags without value as special cases.
                        cargo_args.push(format!("-{flag}"));
                    } else if let Some(val) = parser.optional_value() {
                        cargo_args.push(format!("-{flag}{}", val.string()?));
                    } else {
                        cargo_args.push(format!("-{flag}"));
                    }
                }
                Value(val) => {
                    let val = val.string()?;
                    if subcommand.is_none() {
                        subcommand = Some(val.clone());
                    }
                    cargo_args.push(val);
                }
            }
        }

        term::set_coloring(color.as_deref())?;

        if !exclude.is_empty() && !workspace {
            // TODO: This is the same behavior as cargo, but should we allow it to be used
            // in the root of a virtual workspace as well?
            requires("--exclude", &["--workspace"])?;
        }
        if ignore_unknown_features {
            if features.is_empty() && include_features.is_empty() && group_features.is_empty() {
                requires("--ignore-unknown-features", &[
                    "--features",
                    "--include-features",
                    "--group-features",
                ])?;
            }
            if !include_features.is_empty() {
                let _guard = term::warn::scoped(false);
                // TODO: implement
                warn!(
                    "--ignore-unknown-features for --include-features is not fully implemented and may not work as intended"
                );
            }
            if !group_features.is_empty() {
                let _guard = term::warn::scoped(false);
                // TODO: implement
                warn!(
                    "--ignore-unknown-features for --group-features is not fully implemented and may not work as intended"
                );
            }
        }
        if !each_feature && !feature_powerset {
            if optional_deps.is_some() {
                requires("--optional-deps", &["--each-feature", "--feature-powerset"])?;
            } else if !exclude_features.is_empty() {
                requires("--exclude-features (--skip)", &["--each-feature", "--feature-powerset"])?;
            } else if exclude_no_default_features {
                requires("--exclude-no-default-features", &[
                    "--each-feature",
                    "--feature-powerset",
                ])?;
            } else if exclude_all_features {
                requires("--exclude-all-features", &["--each-feature", "--feature-powerset"])?;
            } else if !include_features.is_empty() {
                requires("--include-features", &["--each-feature", "--feature-powerset"])?;
            } else if include_deps_features {
                requires("--include-deps-features", &["--each-feature", "--feature-powerset"])?;
            } else if !at_least_one_of.is_empty() {
                requires("--at-least-one-of", &["--feature-powerset"])?;
            }
        }

        if !at_least_one_of.is_empty() {
            // there will always be a feature set
            exclude_no_default_features = true;
        }

        if !feature_powerset {
            if depth.is_some() {
                requires("--depth", &["--feature-powerset"])?;
            } else if !group_features.is_empty() {
                requires("--group-features", &["--feature-powerset"])?;
            }
        }

        let depth = depth.as_deref().map(str::parse::<usize>).transpose()?;
        let group_features = parse_grouped_features(&group_features, "group-features")?;
        let at_least_one_of = parse_grouped_features(&at_least_one_of, "at-least-one-of")?;

        if let Some(subcommand) = subcommand.as_deref() {
            match subcommand {
                "test" | "bench" => {
                    if remove_dev_deps {
                        bail!(
                            "--remove-dev-deps may not be used together with {subcommand} subcommand",
                        );
                    } else if no_dev_deps {
                        bail!(
                            "--no-dev-deps may not be used together with {subcommand} subcommand",
                        );
                    }
                }
                // cargo-hack may not be used together with subcommands that do not have the --manifest-path flag.
                "install" => {
                    bail!("cargo-hack may not be used together with {subcommand} subcommand")
                }
                _ => {}
            }
        }

        if let Some(pos) = cargo_args.iter().position(|a| match &**a {
            "--example" | "--examples" | "--test" | "--tests" | "--bench" | "--benches"
            | "--all-targets" => true,
            _ => {
                a.starts_with("--example=") || a.starts_with("--test=") || a.starts_with("--bench=")
            }
        }) {
            if remove_dev_deps {
                conflicts("--remove-dev-deps", &cargo_args[pos])?;
            } else if no_dev_deps {
                conflicts("--no-dev-deps", &cargo_args[pos])?;
            }
        }

        if !include_features.is_empty() {
            if optional_deps.is_some() {
                conflicts("--include-features", "--optional-deps")?;
            } else if include_deps_features {
                conflicts("--include-features", "--include-deps-features")?;
            }
        }

        if no_dev_deps && remove_dev_deps {
            conflicts("--no-dev-deps", "--remove-dev-deps")?;
        }
        if each_feature && feature_powerset {
            conflicts("--each-feature", "--feature-powerset")?;
        }
        if all_features {
            if each_feature {
                conflicts("--all-features", "--each-feature")?;
            } else if feature_powerset {
                conflicts("--all-features", "--feature-powerset")?;
            }
        }
        if no_default_features {
            if each_feature {
                conflicts("--no-default-features", "--each-feature")?;
            } else if feature_powerset {
                conflicts("--no-default-features", "--feature-powerset")?;
            }
        }

        for f in &exclude_features {
            if features.contains(f) {
                bail!("feature `{f}` specified by both --exclude-features and --features");
            }
            if optional_deps.as_ref().map_or(false, |d| d.contains(f)) {
                bail!("feature `{f}` specified by both --exclude-features and --optional-deps");
            }
            if group_features.iter().any(|v| v.matches(f)) {
                bail!("feature `{f}` specified by both --exclude-features and --group-features");
            }
            if include_features.contains(f) {
                bail!("feature `{f}` specified by both --exclude-features and --include-features");
            }
        }

        if subcommand.is_none() {
            if cargo_args.iter().any(|a| a == "--list") {
                cmd!(cargo, "--list").run()?;
                std::process::exit(0);
            } else if !remove_dev_deps {
                // TODO: improve this
                mini_usage("no subcommand or valid flag specified")?;
            }
        }

        if version_range.is_some() {
            let rustup = Rustup::new();
            if rustup.version < 23 {
                bail!("--version-range requires rustup 1.23 or later");
            }
        } else {
            if version_step.is_some() {
                requires("--version-step", &["--version-range"])?;
            }
            if clean_per_version {
                requires("--clean-per-version", &["--version-range"])?;
            }
        }

        if no_dev_deps {
            info!(
                "--no-dev-deps removes dev-dependencies from real `Cargo.toml` while cargo-hack is running and restores it when finished"
            );
        }

        // https://github.com/taiki-e/cargo-hack/issues/42
        // https://github.com/rust-lang/cargo/pull/8799
        let namespaced_features = has_z_flag(&cargo_args, "namespaced-features");
        exclude_no_default_features |= !include_features.is_empty();
        exclude_all_features |= !include_features.is_empty()
            || !exclude_features.is_empty()
            || (feature_powerset && !namespaced_features && depth.is_none());
        exclude_features.extend_from_slice(&features);

        term::verbose::set(verbose != 0);
        // If `-vv` is passed, propagate `-v` to cargo.
        if verbose > 1 {
            cargo_args.push(format!("-{}", "v".repeat(verbose - 1)));
        }

        Ok(Self {
            leading_args: cargo_args,
            trailing_args: rest,

            subcommand,

            manifest_path,
            package,
            exclude,
            workspace,
            each_feature,
            feature_powerset,
            no_dev_deps,
            remove_dev_deps,
            no_private,
            ignore_private: ignore_private | no_private,
            ignore_unknown_features,
            optional_deps,
            clean_per_run,
            clean_per_version,
            keep_going,
            print_command_list,
            no_manifest_path,
            include_features: include_features.into_iter().map(Into::into).collect(),
            at_least_one_of,
            include_deps_features,
            version_range,
            version_step,

            depth,
            group_features,

            exclude_features,
            exclude_no_default_features,
            exclude_all_features,

            features,

            no_default_features,
            target: target.into_iter().collect(),
        })
    }
}

fn parse_grouped_features(
    group_features: &[String],
    option_name: &str,
) -> Result<Vec<Feature>, anyhow::Error> {
    let group_features =
        group_features.iter().try_fold(Vec::with_capacity(group_features.len()), |mut v, g| {
            let g = if g.contains(',') {
                g.split(',')
            } else if g.contains(' ') {
                g.split(' ')
            } else {
                bail!(
                    "--{option_name} requires a list of two or more features separated by space \
                         or comma"
                );
            };
            v.push(Feature::group(g));
            Ok(v)
        })?;
    Ok(group_features)
}

fn has_z_flag(args: &[String], name: &str) -> bool {
    let mut iter = args.iter().map(String::as_str);
    while let Some(mut arg) = iter.next() {
        if arg == "-Z" {
            arg = iter.next().unwrap();
        } else if let Some(a) = arg.strip_prefix("-Z") {
            arg = a;
        } else {
            continue;
        }
        if let Some(rest) = arg.strip_prefix(name) {
            if rest.is_empty() || rest.starts_with('=') {
                return true;
            }
        }
    }
    false
}

// (short flag, long flag, value name, short descriptions, additional descriptions)
type HelpText<'a> = (&'a str, &'a str, &'a str, &'a str, &'a [&'a str]);

const HELP: &[HelpText<'_>] = &[
    ("-p", "--package", "<SPEC>...", "Package(s) to check", &[]),
    ("", "--all", "", "Alias for --workspace", &[]),
    ("", "--workspace", "", "Perform command for all packages in the workspace", &[]),
    ("", "--exclude", "<SPEC>...", "Exclude packages from the check", &[
        "This flag can only be used together with --workspace",
    ]),
    ("", "--manifest-path", "<PATH>", "Path to Cargo.toml", &[]),
    ("-F", "--features", "<FEATURES>...", "Space or comma separated list of features to activate", &[]),
    ("", "--each-feature", "", "Perform for each feature of the package", &[
        "This also includes runs with just --no-default-features flag, and default features.",
        "When this flag is not used together with --exclude-features (--skip) and \
         --include-features and there are multiple features, this also includes runs with \
         just --all-features flag."
    ]),
    ("", "--feature-powerset", "", "Perform for the feature powerset of the package", &[
        "This also includes runs with just --no-default-features flag, and default features.",
        // https://github.com/rust-lang/cargo/pull/8799
        "When this flag is used together with --depth or namespaced features \
         (-Z namespaced-features) and not used together with --exclude-features (--skip) and \
         --include-features and there are multiple features, this also includes runs with just \
         --all-features flag."
    ]),
    ("", "--optional-deps", "[DEPS]...", "Use optional dependencies as features", &[
        "If DEPS are not specified, all optional dependencies are considered as features.",
        "This flag can only be used together with either --each-feature flag or --feature-powerset \
         flag.",
    ]),
    ("", "--skip", "<FEATURES>...", "Alias for --exclude-features", &[]),
    ("", "--exclude-features", "<FEATURES>...", "Space or comma separated list of features to exclude", &[
        "To exclude run of default feature, using value `--exclude-features default`.",
        "To exclude run of just --no-default-features flag, using --exclude-no-default-features \
         flag.",
        "To exclude run of just --all-features flag, using --exclude-all-features flag.",
        "This flag can only be used together with either --each-feature flag or --feature-powerset \
         flag.",
    ]),
    ("", "--exclude-no-default-features", "", "Exclude run of just --no-default-features flag", &[
        "This flag can only be used together with either --each-feature flag or --feature-powerset \
         flag.",
    ]),
    ("", "--exclude-all-features", "", "Exclude run of just --all-features flag", &[
        "This flag can only be used together with either --each-feature flag or --feature-powerset \
         flag.",
    ]),
    (
        "",
        "--depth",
        "<NUM>",
        "Specify a max number of simultaneous feature flags of --feature-powerset",
        &[
            "If NUM is set to 1, --feature-powerset is equivalent to --each-feature.",
            "This flag can only be used together with --feature-powerset flag.",
        ],
    ),
    ("", "--group-features", "<FEATURES>...", "Space or comma separated list of features to group", &[
        "To specify multiple groups, use this option multiple times: `--group-features a,b \
         --group-features c,d`",
        "This flag can only be used together with --feature-powerset flag.",
    ]),
    ("", "--at-least-one-of", "<FEATURES>...", "Space or comma separated list of features. Skips sets of features that don't enable any of the features listed", &[
        "To specify multiple groups, use this option multiple times: `--at-least-one-of a,b \
         --at-least-one-of c,d`",
        "This flag can only be used together with --feature-powerset flag.",
    ]),
    (
        "",
        "--include-features",
        "<FEATURES>...",
        "Include only the specified features in the feature combinations instead of package \
         features",
        &[
            "This flag can only be used together with either --each-feature flag or \
             --feature-powerset flag.",
        ],
    ),
    ("", "--no-dev-deps", "", "Perform without dev-dependencies", &[
        "Note that this flag removes dev-dependencies from real `Cargo.toml` while cargo-hack is \
         running and restores it when finished.",
    ]),
    (
        "",
        "--remove-dev-deps",
        "",
        "Equivalent to --no-dev-deps flag except for does not restore the original `Cargo.toml` \
         after performed",
        &[],
    ),
    ("", "--no-private", "", "Perform without `publish = false` crates", &[]),
    ("", "--ignore-private", "", "Skip to perform on `publish = false` packages", &[]),
    (
        "",
        "--ignore-unknown-features",
        "",
        "Skip passing --features flag to `cargo` if that feature does not exist in the package",
        &["This flag can only be used together with either --features or --include-features."],
    ),
    (
        "",
        "--version-range",
        "[START]..[=END]",
        "Perform commands on a specified (inclusive) range of Rust versions",
        &[
            "If the upper bound of the range is omitted, the latest stable compiler is used as the \
             upper bound.",
            "If the lower bound of the range is omitted, the value of the `rust-version` field in \
             `Cargo.toml` is used as the lower bound.",
            "Note that ranges are always inclusive ranges.",
        ],
    ),
    (
        "",
        "--version-step",
        "<NUM>",
        "Specify the version interval of --version-range (default to `1`)",
        &["This flag can only be used together with --version-range flag."],
    ),
    ("", "--clean-per-run", "", "Remove artifacts for that package before running the command", &[
        "If used this flag with --workspace, --each-feature, or --feature-powerset, artifacts will \
         be removed before each run.",
        "Note that dependencies artifacts will be preserved.",
    ]),
    ("", "--clean-per-version", "", "Remove artifacts per Rust version", &[
        "Note that dependencies artifacts will also be removed.",
        "This flag can only be used together with --version-range flag.",
    ]),
    ("", "--keep-going", "", "Keep going on failure", &[]),
    ("", "--print-command-list", "", "Print commands without run (Unstable)", &[]),
    ("", "--no-manifest-path", "", "Do not pass --manifest-path option to cargo (Unstable)", &[]),
    ("-v", "--verbose", "", "Use verbose output", &[]),
    ("", "--color", "<WHEN>", "Coloring: auto, always, never", &[
        "This flag will be propagated to cargo.",
    ]),
    ("-h", "--help", "", "Prints help information", &[]),
    ("-V", "--version", "", "Prints version information", &[]),
];

struct Help {
    long: bool,
    term_size: usize,
    print_version: bool,
}

const MAX_TERM_WIDTH: usize = 100;

impl Help {
    fn long() -> Self {
        Self { long: true, term_size: MAX_TERM_WIDTH, print_version: true }
    }

    fn short() -> Self {
        Self { long: false, term_size: MAX_TERM_WIDTH, print_version: true }
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
                    write!(f, "{s}")?;
                    written = s.len();
                } else if written == 0 {
                    write!(f, "{s}")?;
                    written += s.len();
                } else {
                    write!(f, " {s}")?;
                    written += s.len() + 1;
                }
            }
            Ok(())
        }

        writeln!(
            f,
            "\
{0}{1}\n{2}
USAGE:
    cargo hack [OPTIONS] [SUBCOMMAND]\n
Use -h for short descriptions and --help for more details.\n
OPTIONS:",
            env!("CARGO_PKG_NAME"),
            if self.print_version { concat!(" ", env!("CARGO_PKG_VERSION")) } else { "" },
            env!("CARGO_PKG_DESCRIPTION")
        )?;

        for &(short, long, value_name, desc, additional) in HELP {
            write!(f, "    {short:2}{} ", if short.is_empty() { " " } else { "," })?;
            if self.long {
                if value_name.is_empty() {
                    writeln!(f, "{long}")?;
                } else {
                    writeln!(f, "{long} {value_name}")?;
                }
                write(f, 12, true, self.term_size, desc)?;
                writeln!(f, ".\n")?;
                for desc in additional {
                    write(f, 12, true, self.term_size, desc)?;
                    writeln!(f, "\n")?;
                }
            } else {
                if value_name.is_empty() {
                    write!(f, "{long:32} ")?;
                } else {
                    let long = format!("{long} {value_name}");
                    write!(f, "{long:32} ")?;
                }
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

// Note: When adding a flag here, update the test with the same name in `tests/test.rs` file.

fn removed_flags(flag: &str) -> Result<()> {
    let alt = match flag {
        "ignore-non-exist-features" => "--ignore-unknown-features",
        "skip-no-default-features" => "--exclude-no-default-features",
        _ => return Ok(()),
    };
    bail!("--{flag} was removed, use {alt} instead")
}

#[cold]
#[inline(never)]
fn similar_arg(flag: &lexopt::Arg<'_>, subcommand: Option<&str>, expected: &str) -> Result<()> {
    let flag = &format_flag(flag);
    bail!(
        "\
Found argument '{flag}' which wasn't expected, or isn't valid in this context
        Did you mean {expected}?

USAGE:
    cargo hack{} {expected}

For more information try --help
",
        subcommand.map_or_else(String::new, |subcommand| String::from(" ") + subcommand),
    )
}

// detect similar flags
fn similar_flags(flag: &str, subcommand: Option<&str>) -> Result<()> {
    let expected = match flag {
        "no-dev-dep" => "--no-dev-deps",
        "remove-dev-dep" => "--remove-dev-deps",
        "each-features" => "--each-feature",
        "features-powerset" => "--feature-powerset",
        "exclude-no-default-feature" => "--exclude-no-default-features",
        "exclude-all-feature" => "--exclude-all-features",
        "include-dep-features" | "include-dep-feature" | "include-deps-feature" => {
            "--include-deps-features"
        }
        "ignore-unknown-feature" => "--ignore-unknown-features",
        _ => return Ok(()),
    };
    similar_arg(&Long(flag), subcommand, expected)
}

#[cold]
#[inline(never)]
fn mini_usage(msg: &str) -> Result<()> {
    bail!(
        "\
{msg}

USAGE:
    cargo hack [OPTIONS] [SUBCOMMAND]

For more information try --help",
    )
}

fn get_help(flag: &str) -> Option<&HelpText<'_>> {
    HELP.iter().find(|&(s, l, ..)| *s == flag || *l == flag)
}

enum OwnedFlag {
    Long(String),
    Short(char),
}

impl OwnedFlag {
    fn as_arg(&self) -> lexopt::Arg<'_> {
        match self {
            Self::Long(flag) => Long(flag),
            &Self::Short(flag) => Short(flag),
        }
    }
}

impl From<lexopt::Arg<'_>> for OwnedFlag {
    fn from(arg: lexopt::Arg<'_>) -> Self {
        match arg {
            Long(flag) => Self::Long(flag.to_owned()),
            Short(flag) => Self::Short(flag),
            Value(_) => unreachable!(),
        }
    }
}

fn format_flag(flag: &lexopt::Arg<'_>) -> String {
    match flag {
        Long(flag) => format!("--{flag}"),
        Short(flag) => format!("-{flag}"),
        Value(_) => unreachable!(),
    }
}

#[cold]
#[inline(never)]
fn multi_arg(flag: &lexopt::Arg<'_>, subcommand: Option<&str>) -> Result<()> {
    let flag = &format_flag(flag);
    let arg = get_help(flag).map_or_else(|| flag.to_string(), |arg| format!("{} {}", arg.1, arg.2));
    bail!(
        "\
The argument '{flag}' was provided more than once, but cannot be used multiple times

USAGE:
    cargo hack{} {arg}

For more information try --help
",
        subcommand.map_or_else(String::new, |subcommand| String::from(" ") + subcommand),
    )
}

/// `flag` requires one of `requires`.
#[cold]
#[inline(never)]
fn requires(flag: &str, requires: &[&str]) -> Result<()> {
    let with = match requires.len() {
        0 => unreachable!(),
        1 => requires[0].to_string(),
        2 => format!("either {} or {}", requires[0], requires[1]),
        _ => {
            let mut with = String::new();
            for f in requires.iter().take(requires.len() - 1) {
                with += f;
                with += ", ";
            }
            with += "or ";
            with += requires.last().unwrap();
            with
        }
    };
    bail!("{flag} can only be used together with {with}");
}

#[cold]
#[inline(never)]
fn conflicts(a: &str, b: &str) -> Result<()> {
    bail!("{a} may not be used together with {b}");
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        io::Write,
        panic,
        path::Path,
        process::{Command, Stdio},
    };

    use anyhow::Result;

    use super::Help;
    use crate::fs;

    #[track_caller]
    fn assert_diff(expected_path: impl AsRef<Path>, actual: impl AsRef<str>) {
        let actual = actual.as_ref();
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let manifest_dir =
            manifest_dir.strip_prefix(env::current_dir().unwrap()).unwrap_or(manifest_dir);
        let expected_path = &manifest_dir.join(expected_path);
        if !expected_path.is_file() {
            fs::write(expected_path, "").unwrap();
        }
        let expected = fs::read_to_string(expected_path).unwrap();
        if expected != actual {
            if env::var_os("CI").is_some() {
                let mut child = Command::new("git")
                    .args(["--no-pager", "diff", "--no-index", "--"])
                    .arg(expected_path)
                    .arg("-")
                    .stdin(Stdio::piped())
                    .spawn()
                    .unwrap();
                child.stdin.as_mut().unwrap().write_all(actual.as_bytes()).unwrap();
                assert!(!child.wait().unwrap().success());
                // patch -p1 <<'EOF' ... EOF
                panic!("assertion failed; please run test locally and commit resulting changes, or apply above diff as patch");
            } else {
                fs::write(expected_path, actual).unwrap();
            }
        }
    }

    #[test]
    fn long_help() {
        let actual = Help { print_version: false, ..Help::long() }.to_string();
        assert_diff("tests/long-help.txt", actual);
    }

    #[test]
    fn short_help() {
        let actual = Help { print_version: false, ..Help::short() }.to_string();
        assert_diff("tests/short-help.txt", actual);
    }

    #[test]
    fn update_readme() -> Result<()> {
        let new = Help { print_version: false, ..Help::long() }.to_string();
        let path = &Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
        let base = fs::read_to_string(path)?;
        let mut out = String::with_capacity(base.capacity());
        let mut lines = base.lines();
        let mut start = false;
        let mut end = false;
        while let Some(line) = lines.next() {
            out.push_str(line);
            out.push('\n');
            if line == "<!-- readme-long-help:start -->" {
                start = true;
                out.push_str("```console\n");
                out.push_str("$ cargo hack --help\n");
                out.push_str(&new);
                for line in &mut lines {
                    if line == "<!-- readme-long-help:end -->" {
                        out.push_str("```\n");
                        out.push_str(line);
                        out.push('\n');
                        end = true;
                        break;
                    }
                }
            }
        }
        if start && end {
            assert_diff(path, out);
        } else if start {
            panic!("missing `<!-- readme-long-help:end -->` comment in README.md");
        } else {
            panic!("missing `<!-- readme-long-help:start -->` comment in README.md");
        }
        Ok(())
    }
}
