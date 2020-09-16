use std::{ffi::OsStr, fmt::Write, ops::Deref};

use crate::{metadata, Args, Info, Manifest, ProcessBuilder, Result};

pub(crate) struct Package<'a> {
    package: &'a metadata::Package,
    pub(crate) manifest: Manifest,
    pub(crate) kind: Kind<'a>,
}

impl<'a> Package<'a> {
    fn new(args: &'a Args, total: &mut usize, package: &'a metadata::Package) -> Result<Self> {
        let manifest = Manifest::new(&package.manifest_path)?;

        if args.ignore_private && manifest.is_private() {
            Ok(Self { package, manifest, kind: Kind::SkipAsPrivate })
        } else {
            Ok(Self { package, manifest, kind: Kind::determine(args, package, total) })
        }
    }

    pub(crate) fn from_iter(
        args: &'a Args,
        total: &mut usize,
        packages: impl IntoIterator<Item = &'a metadata::Package>,
    ) -> Result<Vec<Self>> {
        packages
            .into_iter()
            .map(|package| Package::new(args, total, package))
            .collect::<Result<Vec<_>>>()
    }
}

impl Deref for Package<'_> {
    type Target = metadata::Package;

    fn deref(&self) -> &Self::Target {
        self.package
    }
}

pub(crate) enum Kind<'a> {
    // If there is no subcommand, then kind need not be determined.
    NoSubcommand,
    SkipAsPrivate,
    Nomal { show_progress: bool },
    Each { features: Vec<&'a String> },
    Powerset { features: Vec<Vec<&'a String>> },
}

impl<'a> Kind<'a> {
    fn determine(args: &'a Args, package: &'a metadata::Package, total: &mut usize) -> Self {
        if args.subcommand.is_none() {
            return Kind::NoSubcommand;
        }

        if !args.each_feature && !args.feature_powerset {
            *total += 1;
            return Kind::Nomal { show_progress: false };
        }

        let features =
            package.features.keys().filter(|f| *f != "default" && !args.skip.contains(f));
        let opt_deps = if args.optional_deps {
            Some(
                package
                    .dependencies
                    .iter()
                    .filter_map(|dep| dep.as_feature())
                    .filter(|f| !args.skip.contains(f)),
            )
        } else {
            None
        };

        if args.each_feature {
            let features: Vec<_> = if let Some(opt_deps) = opt_deps {
                features.chain(opt_deps).collect()
            } else {
                features.collect()
            };

            if package.features.is_empty() && features.is_empty() {
                *total += 1;
                Kind::Nomal { show_progress: true }
            } else {
                *total += features.len();
                if !args.skip.iter().any(|x| x == "default") {
                    *total += 1;
                }
                if !args.skip_no_default_features {
                    *total += 1;
                }
                Kind::Each { features }
            }
        } else if args.feature_powerset {
            let features = if let Some(opt_deps) = opt_deps {
                powerset(features.chain(opt_deps))
            } else {
                powerset(features)
            };

            if package.features.is_empty() && features.is_empty() {
                *total += 1;
                Kind::Nomal { show_progress: true }
            } else {
                // -1: the first element of a powerset is `[]`
                *total += features.len() - 1;
                if !args.skip.iter().any(|x| x == "default") {
                    *total += 1;
                }
                if !args.skip_no_default_features {
                    *total += 1;
                }
                Kind::Powerset { features }
            }
        } else {
            unreachable!()
        }
    }
}

pub(crate) fn exec(
    args: &Args,
    package: &Package<'_>,
    line: &ProcessBuilder,
    info: &mut Info,
) -> Result<()> {
    match &package.kind {
        Kind::NoSubcommand => return Ok(()),
        Kind::SkipAsPrivate => unreachable!(),
        Kind::Nomal { show_progress } => {
            // only run with default features
            return exec_cargo(args, package, line, info, *show_progress);
        }
        Kind::Each { .. } | Kind::Powerset { .. } => {}
    }

    let mut line = line.clone();

    if !args.skip.iter().any(|x| x == "default") {
        // run with default features
        exec_cargo(args, package, &line, info, true)?;
    }

    line.arg("--no-default-features");

    if !args.skip_no_default_features {
        // run with no default features if the package has other features
        //
        // `default` is not skipped because `cfg(feature = "default")` is work
        // if `default` feature specified.
        exec_cargo(args, package, &line, info, true)?;
    }

    match &package.kind {
        Kind::Each { features } => features
            .iter()
            .try_for_each(|f| exec_cargo_with_features(args, package, &line, info, Some(f))),
        Kind::Powerset { features } => {
            // The first element of a powerset is `[]` so it should be skipped.
            features
                .iter()
                .skip(1)
                .try_for_each(|f| exec_cargo_with_features(args, package, &line, info, f))
        }
        _ => unreachable!(),
    }
}

fn exec_cargo_with_features(
    args: &Args,
    package: &Package<'_>,
    line: &ProcessBuilder,
    info: &mut Info,
    features: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<()> {
    let mut line = line.clone();
    line.append_features(features);
    exec_cargo(args, package, &line, info, true)
}

fn exec_cargo(
    args: &Args,
    package: &Package<'_>,
    line: &ProcessBuilder,
    info: &mut Info,
    show_progress: bool,
) -> Result<()> {
    info.count += 1;

    if args.clean_per_run {
        cargo_clean(line.get_program(), args, package)?;
    }

    // running `<command>` on <package> (<count>/<total>)
    let mut msg = String::new();
    write!(msg, "running {} on {}", line, &package.name).unwrap();
    if show_progress {
        write!(msg, " ({}/{})", info.count, info.total).unwrap();
    }
    info!(args.color, "{}", msg);

    line.exec()
}

fn cargo_clean(cargo: &OsStr, args: &Args, package: &Package<'_>) -> Result<()> {
    let mut line = ProcessBuilder::new(cargo, args.verbose);
    line.arg("clean");
    line.arg("--package");
    line.arg(&package.name);

    if args.verbose {
        // running `cargo clean --package <package>`
        info!(args.color, "running {}", line);
    }

    line.exec()
}

fn powerset<T: Clone>(iter: impl IntoIterator<Item = T>) -> Vec<Vec<T>> {
    iter.into_iter().fold(vec![vec![]], |mut acc, elem| {
        let ext = acc.clone().into_iter().map(|mut curr| {
            curr.push(elem.clone());
            curr
        });
        acc.extend(ext);
        acc
    })
}
