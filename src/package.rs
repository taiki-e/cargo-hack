use std::{fmt::Write, ops::Deref};

use crate::{metadata, Args, Info, Manifest, ProcessBuilder, Result};

pub(crate) fn features(
    args: &Args,
    package: &Package<'_>,
    line: &ProcessBuilder,
    info: &mut Info,
) -> Result<()> {
    Features { args, package, line: line.clone(), info }.exec()
}

pub(crate) struct Package<'a> {
    package: &'a metadata::Package,
    pub(crate) manifest: Manifest,
    pub(crate) kind: Kind<'a>,
}

impl<'a> Package<'a> {
    fn new(
        args: &'a Args,
        package: &'a metadata::Package,
        total: &mut usize,
    ) -> Result<Option<Self>> {
        let manifest = Manifest::new(&package.manifest_path)?;

        if args.ignore_private && manifest.is_private() {
            Ok(Some(Self { package, manifest, kind: Kind::Skip }))
        } else if args.subcommand.is_some() {
            let (kind, count) = Kind::collect(args, package);
            *total += count;
            Ok(Some(Self { package, manifest, kind }))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn from_iter(
        args: &'a Args,
        total: &mut usize,
        packages: impl IntoIterator<Item = &'a metadata::Package>,
    ) -> Result<Vec<Self>> {
        packages
            .into_iter()
            .filter_map(|package| Package::new(args, package, total).transpose())
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
    Skip,
    Nomal { show_progress: bool },
    Each { features: Vec<&'a String> },
    Powerset { features: Vec<Vec<&'a String>> },
}

impl<'a> Kind<'a> {
    fn collect(args: &'a Args, package: &'a metadata::Package) -> (Self, usize) {
        if !args.each_feature && !args.feature_powerset {
            return (Kind::Nomal { show_progress: false }, 1);
        }

        let features =
            package.features.keys().filter(|k| (*k != "default" && !args.skip.contains(k)));
        let opt_deps = if args.optional_deps {
            Some(package.dependencies.iter().filter_map(|dep| dep.as_feature()))
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
                return (Kind::Nomal { show_progress: true }, 1);
            }

            // +1: default features
            // +1: no default features
            let total = features.len() + 2;
            (Kind::Each { features }, total)
        } else if args.feature_powerset {
            let features = if let Some(opt_deps) = opt_deps {
                powerset(features.chain(opt_deps))
            } else {
                powerset(features)
            };

            if package.features.is_empty() && features.is_empty() {
                return (Kind::Nomal { show_progress: true }, 1);
            }

            // +1: default features
            // +1: no default features
            // -1: the first element of a powerset is `[]`
            let total = features.len() + 1;
            (Kind::Powerset { features }, total)
        } else {
            unreachable!()
        }
    }
}

struct Features<'a> {
    args: &'a Args,
    package: &'a Package<'a>,
    line: ProcessBuilder,
    info: &'a mut Info,
}

impl Features<'_> {
    fn exec(&mut self) -> Result<()> {
        if let Kind::Nomal { show_progress } = &self.package.kind {
            // run with default features
            return self.exec_cargo(None, *show_progress);
        }

        // run with default features
        self.exec_cargo(None, true)?;

        self.line.arg("--no-default-features");

        // run with no default features if the package has other features
        //
        // `default` is not skipped because `cfg(feature = "default")` is work
        // if `default` feature specified.
        self.exec_cargo(None, true)?;

        match &self.package.kind {
            Kind::Each { features } => {
                features.iter().try_for_each(|f| self.exec_cargo_with_features(Some(f)))
            }
            Kind::Powerset { features } => {
                // The first element of a powerset is `[]` so it should be skipped.
                features.iter().skip(1).try_for_each(|f| self.exec_cargo_with_features(f))
            }
            _ => unreachable!(),
        }
    }

    fn exec_cargo_with_features(
        &mut self,
        features: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<()> {
        let mut line = self.line.clone();
        line.append_features(features);
        self.exec_cargo(Some(&line), true)
    }

    fn exec_cargo(&mut self, line: Option<&ProcessBuilder>, show_progress: bool) -> Result<()> {
        let line = line.unwrap_or(&self.line);
        self.info.count += 1;

        // running `<command>` on <package> (<count>/<total>)
        let mut msg = String::new();
        write!(msg, "running {} on {}", line, &self.package.name).unwrap();
        if show_progress {
            write!(msg, " ({}/{})", self.info.count, self.info.total).unwrap();
        }
        info!(self.args.color, "{}", msg);

        line.exec()
    }
}

fn powerset<'a, T>(iter: impl IntoIterator<Item = &'a T>) -> Vec<Vec<&'a T>> {
    iter.into_iter().fold(vec![vec![]], |mut acc, elem| {
        let ext = acc.clone().into_iter().map(|mut curr| {
            curr.push(elem);
            curr
        });
        acc.extend(ext);
        acc
    })
}
