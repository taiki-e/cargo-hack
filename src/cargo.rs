use anyhow::{format_err, Context as _};
use std::{
    env,
    ffi::{OsStr, OsString},
    str,
};

use crate::{ProcessBuilder, Result};

pub(crate) struct Cargo {
    path: OsString,
    pub(crate) version: u32,
}

impl Cargo {
    pub(crate) fn new() -> Self {
        let path = cargo_binary();

        // If failed to determine cargo version, assign 0 to skip all version-dependent decisions.
        let version = cargo_minor_version(&path)
            .map_err(|e| warn!("unable to determine cargo version: {}", e))
            .unwrap_or(0);

        Self { path, version }
    }

    pub(crate) fn process(&self) -> ProcessBuilder<'_> {
        ProcessBuilder::new(&self.path)
    }
}

// Based on https://github.com/cuviper/autocfg/blob/1.0.1/src/version.rs#L25-L59
fn cargo_minor_version(path: &OsStr) -> Result<u32> {
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

    Ok(minor)
}

fn cargo_binary() -> OsString {
    env::var_os("CARGO_HACK_CARGO_SRC")
        .unwrap_or_else(|| env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}
