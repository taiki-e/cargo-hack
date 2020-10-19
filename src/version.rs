use anyhow::{format_err, Context as _};
use std::{ffi::OsStr, str};

use crate::{ProcessBuilder, Result};

pub(crate) struct Version {
    pub(crate) minor: u32,
}

pub(crate) fn from_path(path: &OsStr) -> Result<Version> {
    let mut command = ProcessBuilder::new(path);
    command.args(&["--version", "--verbose"]);
    let output = command.exec_with_output()?;

    let output = str::from_utf8(&output.stdout)
        .with_context(|| format!("failed to parse output of {}", command))?;

    // Find the release line in the verbose version output.
    let release = output
        .lines()
        .find(|line| line.starts_with("release: "))
        .map(|line| &line["release: ".len()..])
        .ok_or_else(|| format_err!("could not find rustc release from output of {}", command))?;

    // Split the version and channel info.
    let mut version_channel = release.split('-');
    let version = version_channel.next().unwrap();
    let _channel = version_channel.next();

    let minor = (|| {
        // Split the version into semver components.
        let mut digits = version.splitn(3, '.');
        let major = digits.next()?;
        if major != "1" {
            return None;
        }
        let minor = digits.next()?.parse().ok()?;
        let _patch = digits.next()?;
        Some(minor)
    })()
    .ok_or_else(|| format_err!("unexpected output from {}", command))?;

    Ok(Version { minor })
}
