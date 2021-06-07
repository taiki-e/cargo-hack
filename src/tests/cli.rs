use std::{env, path::Path, process::Command};

use anyhow::Result;
use tempfile::Builder;

use super::Help;

#[track_caller]
fn assert_diff(expected_path: impl AsRef<Path>, actual: impl AsRef<str>) {
    let actual = actual.as_ref();
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expected_path = &manifest_dir.join(expected_path);
    let expected = fs::read_to_string(expected_path).unwrap();
    if expected != actual {
        if env::var_os("CI").is_some() {
            let outdir = Builder::new().prefix("assert_diff").tempdir().unwrap();
            let actual_path = &outdir.path().join(expected_path.file_name().unwrap());
            fs::write(actual_path, actual).unwrap();
            let status = Command::new("git")
                .args(&["--no-pager", "diff", "--no-index", "--"])
                .args(&[expected_path, actual_path])
                .status()
                .unwrap();
            assert!(!status.success());
            panic!("assertion failed");
        } else {
            fs::write(expected_path, actual).unwrap();
        }
    }
}

#[test]
fn long_help() {
    let actual = Help { long: true, term_size: 200, print_version: false }.to_string();
    assert_diff("tests/long-help.txt", actual);
}

#[test]
fn short_help() {
    let actual = Help { long: false, term_size: 200, print_version: false }.to_string();
    assert_diff("tests/short-help.txt", actual);
}

#[test]
fn update_readme() -> Result<()> {
    let new = Help { long: true, term_size: 80, print_version: false }.to_string();
    let path = &Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
    let base = fs::read_to_string(path)?;
    let mut out = String::with_capacity(base.capacity());
    let mut lines = base.lines();
    let mut start = false;
    let mut end = false;
    while let Some(line) = lines.next() {
        dbg!(&line);
        out.push_str(line);
        out.push('\n');
        if line == "<!-- readme-long-help:start -->" {
            start = true;
            out.push_str("```console\n");
            out.push_str("$ cargo hack --help\n");
            out.push_str(&new);
            out.push('\n');
            for line in &mut lines {
                if line == "<!-- readme-long-help:end -->" {
                    out.push_str("```\n");
                    out.push_str(line);
                    out.push('\n');
                    end = true;
                    break;
                }
            }
        }
    }
    if start && end {
        fs::write(path, out)?;
    } else if start {
        panic!("do not modify `<!-- readme-long-help:end -->` comment in README.md")
    } else {
        panic!("do not modify `<!-- readme-long-help:start -->` comment in README.md")
    }
    Ok(())
}
