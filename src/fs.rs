use std::path::Path;

use anyhow::{Context as _, Result};

/// Write a slice as the entire contents of a file.
/// This is a wrapper for [`std::fs::write`].
pub(crate) fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<()> {
    let path = path.as_ref();
    let res = std::fs::write(path, contents.as_ref());
    res.with_context(|| format!("failed to write to file `{}`", path.display()))
}

/// Read the entire contents of a file into a string.
/// This is a wrapper for [`std::fs::read_to_string`].
pub(crate) fn read_to_string(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    let res = std::fs::read_to_string(path);
    res.with_context(|| format!("failed to read from file `{}`", path.display()))
}
