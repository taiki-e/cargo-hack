use std::{fmt, str::FromStr};

use anyhow::{bail, Context as _, Error, Result};

#[derive(Copy, Clone)]
pub(crate) struct Version {
    pub(crate) major: u32,
    pub(crate) minor: u32,
    pub(crate) patch: Option<u32>,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let major = self.major;
        let minor = self.minor;
        write!(f, "{major}.{minor}")?;
        if let Some(patch) = self.patch {
            write!(f, ".{patch}")?;
        }
        Ok(())
    }
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

#[derive(Copy, Clone)]
pub(crate) enum MaybeVersion {
    Version(Version),
    Msrv,
    Stable,
}

#[derive(Copy, Clone)]
pub(crate) struct VersionRange {
    pub(crate) start_inclusive: MaybeVersion,
    pub(crate) end_inclusive: MaybeVersion,
}

impl VersionRange {
    pub(crate) fn msrv() -> Self {
        Self { start_inclusive: MaybeVersion::Msrv, end_inclusive: MaybeVersion::Msrv }
    }
}

impl fmt::Display for VersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let MaybeVersion::Version(start) = self.start_inclusive {
            write!(f, "{start}")?;
        }
        write!(f, "..")?;
        if let MaybeVersion::Version(end) = self.end_inclusive {
            write!(f, "={end}")?;
        }
        Ok(())
    }
}

impl FromStr for VersionRange {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (start, end) = if let Some((start, end)) = s.split_once("..") {
            let end = match end.strip_prefix('=') {
                Some(end) => {
                    if end.is_empty() {
                        // Reject inclusive range without end expression (`..=` and `<start>..=`). (same behavior as Rust's inclusive range)
                        bail!(
                            "inclusive range `{s}` must have end expression; consider using `{}` or `{s}<end>`",
                            s.replace("..=", "..")
                        )
                    }
                    end
                }
                None => {
                    // `..` and `<start>..` are okay, so only warn `<start>..<end>`.
                    if !end.is_empty() {
                        warn!(
                            "using `..` for inclusive range is deprecated; consider using `{}`",
                            s.replace("..", "..=")
                        );
                    }
                    end
                }
            };
            (start, maybe_version(end)?)
        } else {
            (s, None)
        };
        let start_inclusive = maybe_version(start)?.unwrap_or(MaybeVersion::Msrv);
        let end_inclusive = end.unwrap_or(MaybeVersion::Stable);
        Ok(Self { start_inclusive, end_inclusive })
    }
}

fn maybe_version(s: &str) -> Result<Option<MaybeVersion>, Error> {
    if s.is_empty() {
        Ok(None)
    } else {
        s.parse().map(MaybeVersion::Version).map(Some)
    }
}
