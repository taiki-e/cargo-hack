// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{io::Write, path::Path};

use anyhow::{Context as _, Result};

// TODO: we handle SIGINT, so we have no problem such as https://github.com/rust-lang/cargo/issues/11386
// but maybe useful for a case like https://github.com/rust-lang/cargo/issues/12704
// Adapted from https://github.com/rust-lang/cargo/blob/b31577d43ce235bb77167d399e14a0b5f6fdf584/crates/cargo-util/src/paths.rs#L186.
/// Writes a file to disk atomically.
pub(crate) fn write_atomic(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<()> {
    let path = path.as_ref();

    // On unix platforms, get the permissions of the original file. Copy only the user/group/other
    // read/write/execute permission bits. The tempfile lib defaults to an initial mode of 0o600,
    // and we'll set the proper permissions after creating the file.
    #[cfg(unix)]
    let perms = path.metadata().ok().map(|meta| {
        use std::os::unix::fs::PermissionsExt;

        // these constants are u16 on macOS
        let mask = u32::from(libc::S_IRWXU | libc::S_IRWXG | libc::S_IRWXO);
        let mode = meta.permissions().mode() & mask;

        std::fs::Permissions::from_mode(mode)
    });

    let mut tmp = tempfile::Builder::new()
        .prefix(path.file_name().unwrap())
        .tempfile_in(path.parent().unwrap())?;
    tmp.write_all(contents.as_ref())?;

    // On unix platforms, set the permissions on the newly created file. We can use fchmod (called
    // by the std lib; subject to change) which ignores the umask so that the new file has the same
    // permissions as the old file.
    #[cfg(unix)]
    if let Some(perms) = perms {
        tmp.as_file().set_permissions(perms)?;
    }

    tmp.persist(path)?;
    Ok(())
}

/// Read the entire contents of a file into a string.
/// This is a wrapper for [`std::fs::read_to_string`].
pub(crate) fn read_to_string(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    let res = std::fs::read_to_string(path);
    res.with_context(|| format!("failed to read from file `{}`", path.display()))
}
