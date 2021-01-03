use anyhow::{Context as _, Result};

pub(crate) struct Version {
    pub(crate) major: u32,
    pub(crate) minor: u32,
    pub(crate) patch: Option<u32>,
}

pub(crate) fn parse_version(s: &str) -> Result<Version> {
    let mut digits = s.splitn(3, '.');
    let major = digits.next().context("missing major version")?.parse()?;
    let minor = digits.next().context("missing minor version")?.parse()?;
    let patch = digits.next().map(str::parse).transpose()?;
    Ok(Version { major, minor, patch })
}
