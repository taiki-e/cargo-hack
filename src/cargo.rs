// SPDX-License-Identifier: Apache-2.0 OR MIT

use anyhow::{Result, bail, format_err};

use crate::{ProcessBuilder, metadata::Package, version::Version};

pub(crate) fn version(mut cmd: ProcessBuilder<'_>) -> Result<Version> {
    // Use verbose version output because the packagers add extra strings to the normal version output.
    cmd.arg("-vV");
    let verbose_version = cmd.read()?;
    let release = verbose_version
        .lines()
        .find_map(|line| line.strip_prefix("release: "))
        .ok_or_else(|| format_err!("unexpected output from {cmd}: {verbose_version}"))?;
    let (version, _channel) = release.split_once('-').unwrap_or((release, ""));

    let version: Version = version.parse()?;
    if version.major != 1 || version.patch.is_none() {
        bail!("unexpected output from {cmd}: {verbose_version}");
    }

    Ok(version)
}

// From cargo-llvm-cov
// TODO: glob pattern
pub(crate) fn match_pkg_spec(pkg: &Package, name_or_spec: &str) -> Result<bool> {
    /*
    Refs: https://doc.rust-lang.org/1.93.0/cargo/reference/pkgid-spec.html
        spec := pkgname |
            [ kind "+" ] proto "://" hostname-and-path [ "?" query] [ "#" ( pkgname | semver ) ]
        query = ( "branch" | "tag" | "rev" ) "=" ref
        pkgname := name [ ("@" | ":" ) semver ]
        semver := digits [ "." digits [ "." digits [ "-" prerelease ] [ "+" build ]]]

        kind = "registry" | "git" | "path"
        proto := "http" | "git" | "file" | ...
    */
    fn split_spec(s: &str) -> Option<(&str, &str, Option<&str>, Option<&str>)> {
        let (proto_etc, hostname_and_path_etc) = s.split_once("://")?;
        let proto = proto_etc.split_once('+').unwrap_or(("", proto_etc)).1; // drop kind
        let (hostname_and_path_etc, pkgname_or_semver) =
            hostname_and_path_etc.split_once('#').unwrap_or((hostname_and_path_etc, ""));
        let (hostname_and_path, query) =
            hostname_and_path_etc.split_once('?').unwrap_or((hostname_and_path_etc, ""));
        Some((
            proto,
            hostname_and_path,
            if query.is_empty() { None } else { Some(query) },
            if pkgname_or_semver.is_empty() { None } else { Some(pkgname_or_semver) },
        ))
    }
    fn split_semver(
        s: &str,
    ) -> Option<(&str, Option<(&str, Option<(&str, Option<&str>, Option<&str>)>)>)> {
        let mut digits = s.splitn(3, '.');
        let major = digits.next()?;
        let Some(minor) = digits.next() else {
            return Some((major, None));
        };
        let Some(patch_etc) = digits.next() else {
            return Some((major, Some((minor, None))));
        };
        let (patch_etc, meta) = patch_etc.split_once('+').unwrap_or((patch_etc, ""));
        let (patch, pre) = patch_etc.split_once('-').unwrap_or((patch_etc, ""));
        Some((
            major,
            Some((
                minor,
                Some((
                    patch,
                    if pre.is_empty() { None } else { Some(pre) },
                    if meta.is_empty() { None } else { Some(meta) },
                )),
            )),
        ))
    }
    let name = &*pkg.name;
    let p = name_or_spec;
    let (version, full_version) = if p.starts_with(name) {
        if p.len() == name.len() {
            return Ok(true); // version omitted
        }
        if !matches!(p.as_bytes().get(name.len()), Some(&b'@' | &b':')) {
            return Ok(false); // pkgname unmatched
        }
        (&p[name.len() + 1..], &*pkg.version)
    } else {
        let p = p.trim_ascii_end(); // pkgid may contains trailing newline (e.g., when pkgid is got from `cargo pkgid -p <package>`)
        let full = &*pkg.id;
        let Some((proto, hostname_and_path, query, pkgname_or_semver)) = split_spec(p) else {
            return Ok(false); // p is not pkg spec
        };
        let Some((full_proto, full_hostname_and_path, full_query, full_pkgname_or_semver)) =
            split_spec(full)
        else {
            bail!("invalid pkg spec ({full}) from cargo-metadata")
        };
        if proto != full_proto || hostname_and_path != full_hostname_and_path {
            return Ok(false); // proto or hostname-and-path unmatched
        }
        if query.is_some() && query != full_query {
            return Ok(false); // query unmatched
        }
        let Some(pkgname_or_semver) = pkgname_or_semver else {
            return Ok(true); // pkgname | semver omitted
        };
        let Some(full_pkgname_or_semver) = full_pkgname_or_semver else {
            return Ok(false); // extra pkgname | semver
        };
        match (
            pkgname_or_semver.split_once(['@', ':']),
            full_pkgname_or_semver.split_once(['@', ':']),
        ) {
            (Some((pkgname, semver)), Some((full_pkgname, full_semver))) => {
                if pkgname != full_pkgname {
                    return Ok(false); // pkgname unmatched
                }
                (semver, full_semver)
            }
            (Some(_), None) => return Ok(false), // extra semver
            (None, _) => return Ok(true),        // pkgname omitted or no pkgname in spec
        }
    };
    let Some((major, minor_etc)) = split_semver(version) else {
        warn!("invalid pkg version ({version}) from --package");
        return Ok(false); // invalid version
    };
    let Some((full_major, Some((full_minor, Some((full_patch, full_pre, full_meta)))))) =
        split_semver(full_version)
    else {
        bail!("invalid pkg version ({full_version}) from cargo-metadata")
    };
    if major != full_major {
        return Ok(false); // major unmatched
    }
    let Some((minor, patch_etc)) = minor_etc else {
        return Ok(true); // minor version omitted
    };
    if minor != full_minor {
        return Ok(false); // minor unmatched
    }
    let Some((patch, pre, meta)) = patch_etc else {
        return Ok(true); // patch version omitted
    };
    if patch != full_patch
        || pre.is_some() && pre != full_pre
        || meta.is_some() && meta != full_meta
    {
        return Ok(false); // patch or pre or meta unmatched
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::Path};

    use super::{Package, match_pkg_spec};

    #[test]
    fn test_match_pkg_spec() {
        // Examples are from https://doc.rust-lang.org/1.93.0/cargo/reference/pkgid-spec.html#example-specifications

        let new_pkg = |id: &str, name: &str, version: &str| Package {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            dependencies: Box::default(),
            features: BTreeMap::new(),
            manifest_path: Path::new("").into(),
            publish: true,
            rust_version: None,
        };

        // crates.io
        let pkg = &new_pkg(
            "registry+https://github.com/rust-lang/crates.io-index#regex@1.4.3",
            "regex",
            "1.4.3",
        );
        // name
        assert!(match_pkg_spec(pkg, "regex").unwrap());
        assert!(!match_pkg_spec(pkg, "regex-syntax").unwrap());
        // name+version
        assert!(match_pkg_spec(pkg, "regex@1").unwrap());
        assert!(match_pkg_spec(pkg, "regex@1.4").unwrap());
        assert!(match_pkg_spec(pkg, "regex@1.4.3").unwrap());
        assert!(match_pkg_spec(pkg, "regex:1.4").unwrap());
        assert!(!match_pkg_spec(pkg, "regex@2").unwrap());
        assert!(!match_pkg_spec(pkg, "regex@1.5").unwrap());
        assert!(!match_pkg_spec(pkg, "regex@1.4.2").unwrap());
        assert!(!match_pkg_spec(pkg, "regex@1.4.4").unwrap());
        // spec
        assert!(match_pkg_spec(pkg, "https://github.com/rust-lang/crates.io-index#regex").unwrap());
        assert!(
            match_pkg_spec(pkg, "https://github.com/rust-lang/crates.io-index#regex@1.4.3")
                .unwrap()
        );
        assert!(
            match_pkg_spec(pkg, "https://github.com/rust-lang/crates.io-index#regex@1.4").unwrap()
        );
        assert!(
            match_pkg_spec(
                pkg,
                "registry+https://github.com/rust-lang/crates.io-index#regex@1.4.3"
            )
            .unwrap()
        );

        // git
        let pkg = &new_pkg(
            "git+ssh://git@github.com/rust-lang/regex.git?branch=dev#regex@1.4.3",
            "regex",
            "1.4.3",
        );
        assert!(match_pkg_spec(pkg, "regex").unwrap());
        assert!(
            match_pkg_spec(pkg, "ssh://git@github.com/rust-lang/regex.git#regex@1.4.3").unwrap()
        );
        assert!(
            match_pkg_spec(pkg, "git+ssh://git@github.com/rust-lang/regex.git#regex@1.4.3")
                .unwrap()
        );
        assert!(
            match_pkg_spec(
                pkg,
                "git+ssh://git@github.com/rust-lang/regex.git?branch=dev#regex@1.4.3"
            )
            .unwrap()
        );
        let pkg = &new_pkg("git+https://github.com/rust-lang/cargo#0.52.0", "cargo", "0.52.0");
        assert!(match_pkg_spec(pkg, "https://github.com/rust-lang/cargo#0.52.0").unwrap());
        assert!(match_pkg_spec(pkg, "git+https://github.com/rust-lang/cargo#0.52.0").unwrap());
        assert!(
            !match_pkg_spec(pkg, "https://github.com/rust-lang/cargo#cargo-platform@0.1.2")
                .unwrap()
        );

        // local
        let pkg = &new_pkg("path+file:///path/to/my/project/foo#1.1.8", "foo", "1.1.8");
        assert!(match_pkg_spec(pkg, "foo").unwrap());
        assert!(match_pkg_spec(pkg, "file:///path/to/my/project/foo").unwrap());
        assert!(match_pkg_spec(pkg, "file:///path/to/my/project/foo#1.1.8").unwrap());
        assert!(match_pkg_spec(pkg, "path+file:///path/to/my/project/foo#1.1").unwrap());
        assert!(match_pkg_spec(pkg, "path+file:///path/to/my/project/foo#1.1.8").unwrap());
    }
}
