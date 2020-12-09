use anyhow::{bail, format_err, Context as _};
use std::str;

use crate::{
    cargo,
    version::{parse_version, Version},
    ProcessBuilder, Result,
};

pub(crate) struct Rustup {
    pub(crate) version: u32,
}

impl Rustup {
    pub(crate) fn new() -> Self {
        // If failed to determine rustup version, assign 0 to skip all version-dependent decisions.
        let version = minor_version()
            .map_err(|e| warn!("unable to determine rustup version: {:#}", e))
            .unwrap_or(0);

        Self { version }
    }
}

pub(crate) fn version_range(range: &str, step: Option<&str>) -> Result<Vec<String>> {
    let check = |version: &Version| {
        if version.major != 1 {
            bail!("major version must be 1");
        }
        if let Some(patch) = version.patch {
            warn!(
                "--version-range always selects the latest patch release per minor release, \
                     not the specified patch release `{}`",
                patch
            )
        }
        Ok(())
    };

    let mut split = range.splitn(2, "..");
    let start = split.next().map(parse_version).unwrap()?;
    check(&start)?;

    let end = match split.next() {
        Some("") | None => {
            install_toolchain("stable", None, false)?;
            cargo::minor_version(ProcessBuilder::new("cargo").args(&["+stable"]))?
        }
        Some(end) => {
            let end = parse_version(end)?;
            check(&end)?;
            end.minor
        }
    };

    let step = step.map(str::parse::<u8>).transpose()?.unwrap_or(1);
    if step == 0 {
        bail!("--version-step cannot be zero");
    }

    let versions: Vec<_> =
        (start.minor..=end).step_by(step as _).map(|minor| format!("+1.{}", minor)).collect();
    if versions.is_empty() {
        bail!("specified version range `{}` is empty", range);
    }
    Ok(versions)
}

pub(crate) fn install_toolchain(
    mut toolchain: &str,
    target: Option<&str>,
    print_output: bool,
) -> Result<()> {
    if toolchain.starts_with('+') {
        toolchain = &toolchain[1..];
    }
    // In Github Actions and Azure Pipelines, --no-self-update is necessary
    // because the windows environment cannot self-update rustup.exe.
    let mut cmd = rustup();
    cmd.args(&["toolchain", "install", toolchain, "--no-self-update"]);
    if let Some(target) = target {
        cmd.args(&["--target", target]);
    }

    if print_output {
        // The toolchain installation can take some time, so we'll show users
        // the progress.
        cmd.exec()
    } else {
        // However, in certain situations, it may be preferable not to display it.
        cmd.exec_with_output().map(drop)
    }
}

fn rustup<'a>() -> ProcessBuilder<'a> {
    ProcessBuilder::new("rustup")
}

fn minor_version() -> Result<u32> {
    let mut cmd = rustup();
    cmd.args(&["--version"]);
    let output = cmd.exec_with_output()?;

    let output = str::from_utf8(&output.stdout)
        .with_context(|| format!("failed to parse output of {}", cmd))?;

    let version = (|| {
        let mut output = output.split(' ');
        if output.next()? != "rustup" {
            return None;
        }
        output.next()
    })()
    .ok_or_else(|| format_err!("unexpected output from {}: {}", cmd, output))?;
    let version = parse_version(version)?;
    if version.major != 1 || version.patch.is_none() {
        bail!("unexpected output from {}: {}", cmd, output);
    }

    Ok(version.minor)
}
