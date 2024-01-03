// SPDX-License-Identifier: Apache-2.0 OR MIT

use anyhow::{bail, format_err, Result};

use crate::{version::Version, ProcessBuilder};

pub(crate) fn version(mut cmd: ProcessBuilder<'_>) -> Result<Version> {
    // Use verbose version output because the packagers add extra strings to the normal version output.
    cmd.args(["--version", "--verbose"]);
    let verbose_version = cmd.read()?;
    let release = verbose_version
        .lines()
        .find_map(|line| line.strip_prefix("release: "))
        .ok_or_else(|| format_err!("unexpected output from {cmd}: {verbose_version}"))?;
    let (version, _channel) = release.split_once('-').unwrap_or((release, ""));

    let version: Version = version.parse()?;
    if version.major != 1 || version.patch.is_none() {
        bail!("unexpected output from {cmd}: {verbose_version}");
    }

    Ok(version)
}
