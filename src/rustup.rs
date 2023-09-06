use std::str;

use anyhow::{bail, format_err, Result};

use crate::{
    cargo,
    context::Context,
    version::{MaybeVersion, Version, VersionRange},
    PackageRuns,
};

pub(crate) struct Rustup {
    pub(crate) version: u32,
}

impl Rustup {
    pub(crate) fn new() -> Self {
        // If failed to determine rustup version, assign 0 to skip all version-dependent decisions.
        let version = minor_version()
            .map_err(|e| warn!("unable to determine rustup version: {e:#}"))
            .unwrap_or(0);

        Self { version }
    }
}

pub(crate) fn version_range(
    range: VersionRange,
    step: u16,
    packages: &[PackageRuns<'_>],
    cx: &Context,
) -> Result<Vec<u32>> {
    let check = |version: &Version| {
        if version.major != 1 {
            bail!("major version must be 1");
        }
        if let Some(patch) = version.patch {
            warn!(
                "--version-range always selects the latest patch release per minor release, \
                 not the specified patch release `{patch}`",
            );
        }
        Ok(())
    };

    let mut stable_version = None;
    let mut get_stable_version = || -> Result<Version> {
        if let Some(stable_version) = stable_version {
            Ok(stable_version)
        } else {
            install_toolchain("stable", &[], false)?;
            let version = cargo::version(cmd!("rustup", "run", "stable", "cargo"))?;
            stable_version = Some(version);
            Ok(version)
        }
    };

    let mut rust_version = None;
    let mut get_rust_version = || -> Result<Version> {
        if let Some(rust_version) = rust_version {
            Ok(rust_version)
        } else {
            let mut version = None;
            for pkg in packages {
                let v = cx.rust_version(pkg.id);
                if v.is_none() || v == version {
                    // no-op
                } else if version.is_none() {
                    version = v;
                } else {
                    bail!("automatic detection of the lower bound of the version range is not yet supported when the minimum supported Rust version of the crates in the workspace do not match")
                }
            }
            let version = match version {
                Some(v) => v.parse()?,
                None => bail!("no rust-version field in Cargo.toml is specified"),
            };
            rust_version = Some(version);
            Ok(version)
        }
    };

    let VersionRange { start_inclusive, end_inclusive } = range;

    let start_inclusive = match start_inclusive {
        MaybeVersion::Version(start) => {
            check(&start)?;
            start
        }
        MaybeVersion::Msrv => {
            let start = get_rust_version()?;
            check(&start)?;
            start
        }
        MaybeVersion::Stable => get_stable_version()?,
    };

    let end_inclusive = match end_inclusive {
        MaybeVersion::Version(end) => {
            check(&end)?;
            end
        }
        MaybeVersion::Msrv => get_rust_version()?,
        MaybeVersion::Stable => get_stable_version()?,
    };

    let versions: Vec<_> =
        (start_inclusive.minor..=end_inclusive.minor).step_by(step as _).collect();
    if versions.is_empty() {
        bail!("specified version range `{range}` is empty");
    }
    Ok(versions)
}

pub(crate) fn install_toolchain(
    mut toolchain: &str,
    target: &[String],
    print_output: bool,
) -> Result<()> {
    toolchain = toolchain.strip_prefix('+').unwrap_or(toolchain);

    if target.is_empty()
        && cmd!("rustup", "run", toolchain, "cargo", "--version").run_with_output().is_ok()
    {
        // Do not run `rustup toolchain add` if the toolchain already has installed.
        return Ok(());
    }

    // In Github Actions and Azure Pipelines, --no-self-update is necessary
    // because the windows environment cannot self-update rustup.exe.
    let mut cmd = cmd!("rustup", "toolchain", "add", toolchain, "--no-self-update");
    if !target.is_empty() {
        cmd.args(["--target", &target.join(",")]);
    }

    if print_output {
        // The toolchain installation can take some time, so we'll show users
        // the progress.
        cmd.run()
    } else {
        // However, in certain situations, it may be preferable not to display it.
        cmd.run_with_output().map(drop)
    }
}

fn minor_version() -> Result<u32> {
    let mut cmd = cmd!("rustup", "--version");
    let output = cmd.read()?;

    let version = (|| {
        let mut output = output.split(' ');
        if output.next()? != "rustup" {
            return None;
        }
        output.next()
    })()
    .ok_or_else(|| format_err!("unexpected output from {cmd}: {output}"))?;
    let version: Version = version.parse()?;
    if version.major != 1 || version.patch.is_none() {
        bail!("unexpected output from {cmd}: {output}");
    }

    Ok(version.minor)
}
