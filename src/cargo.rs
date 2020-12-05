use anyhow::{bail, format_err, Context as _};
use std::{env, ffi::OsString, str};

use crate::{version::parse_version, ProcessBuilder, Result};

pub(crate) struct Cargo {
    path: OsString,
    pub(crate) version: u32,
}

impl Cargo {
    pub(crate) fn new() -> Self {
        let path = cargo_binary();

        // If failed to determine cargo version, assign 0 to skip all version-dependent decisions.
        let version = minor_version(&mut ProcessBuilder::new(&path))
            .map_err(|e| warn!("unable to determine cargo version: {:#}", e))
            .unwrap_or(0);

        Self { path, version }
    }

    pub(crate) fn process(&self) -> ProcessBuilder<'_> {
        ProcessBuilder::new(&self.path)
    }
}

// Based on https://github.com/cuviper/autocfg/blob/1.0.1/src/version.rs#L25-L59
pub(crate) fn minor_version(cmd: &mut ProcessBuilder<'_>) -> Result<u32> {
    cmd.args(&["--version", "--verbose"]);
    let output = cmd.exec_with_output()?;

    let output = str::from_utf8(&output.stdout)
        .with_context(|| format!("failed to parse output of {}", cmd))?;

    // Find the release line in the verbose version output.
    let release = output
        .lines()
        .find(|line| line.starts_with("release: "))
        .map(|line| &line["release: ".len()..])
        .ok_or_else(|| {
            format_err!("could not find rustc release from output of {}: {}", cmd, output)
        })?;

    // Split the version and channel info.
    let mut version_channel = release.split('-');
    let version = version_channel.next().unwrap();
    let _channel = version_channel.next();

    let version = parse_version(version)?;
    if version.major != 1 || version.patch.is_none() {
        bail!("unexpected output from {}: {}", cmd, output);
    }

    Ok(version.minor)
}

fn cargo_binary() -> OsString {
    env::var_os("CARGO_HACK_CARGO_SRC")
        .unwrap_or_else(|| env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}
