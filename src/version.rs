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

pub(crate) struct VersionRange {
    pub(crate) start_inclusive: Option<Version>,
    pub(crate) end_inclusive: Option<Version>,
}

impl FromStr for VersionRange {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (start, end_inclusive) = if let Some((start, end)) = s.split_once("..") {
            let end = match end.strip_prefix('=') {
                Some(end) => end,
                None => {
                    warn!(
                        "using `..` for inclusive range is deprecated; consider using `{}`",
                        s.replace("..", "..=")
                    );
                    end
                }
            };
            (start, maybe_version(end)?)
        } else {
            (s, None)
        };
        let start_inclusive = maybe_version(start)?;
        Ok(Self { start_inclusive, end_inclusive })
    }
}

fn maybe_version(s: &str) -> Result<Option<Version>, Error> {
    if s.is_empty() {
        Ok(None)
    } else {
        s.parse().map(Some)
    }
}
