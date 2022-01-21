use anyhow::{bail, format_err, Result};

use crate::{version::Version, ProcessBuilder};

// The version detection logic is based on https://github.com/cuviper/autocfg/blob/1.0.1/src/version.rs#L25-L59
pub(crate) fn minor_version(mut cmd: ProcessBuilder<'_>) -> Result<u32> {
    cmd.args(&["--version", "--verbose"]);
    let output = cmd.read()?;

    // Find the release line in the verbose version output.
    let release =
        output.lines().find_map(|line| line.strip_prefix("release: ")).ok_or_else(|| {
            format_err!("could not find rustc release from output of {}: {}", cmd, output)
        })?;

    // Split the version and channel info.
    let mut version_channel = release.split('-');
    let version = version_channel.next().unwrap();
    let _channel = version_channel.next();

    let version: Version = version.parse()?;
    if version.major != 1 || version.patch.is_none() {
        bail!("unexpected output from {}: {}", cmd, output);
    }

    Ok(version.minor)
}
