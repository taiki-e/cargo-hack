use std::str::FromStr;

use anyhow::{Context as _, Error, Result};

pub(crate) struct Version {
    pub(crate) major: u32,
    pub(crate) minor: u32,
    pub(crate) patch: Option<u32>,
}

impl FromStr for Version {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut digits = s.splitn(3, '.');
        let major = digits.next().context("missing major version")?.parse()?;
        let minor = digits.next().context("missing minor version")?.parse()?;
        let patch = digits.next().map(str::parse).transpose()?;
        Ok(Self { major, minor, patch })
    }
}
