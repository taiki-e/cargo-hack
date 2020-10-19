#![warn(rust_2018_idioms, single_use_lifetimes)]

use easy_ext::ext;
use std::{
    env,
    path::Path,
    process::{Command, ExitStatus},
};

#[cfg(not(windows))]
const SEPARATOR: char = '/';
#[cfg(windows)]
const SEPARATOR: char = '\\';

fn cargo_hack() -> Command {
    // TODO: update to use CARGO_BIN_EXE (https://github.com/rust-lang/cargo/pull/7697, require Rust 1.43).
    let mut exe = env::current_exe().unwrap();
    exe.pop();
    if exe.ends_with("deps") {
        exe.pop();
    }
    exe.push("cargo-hack");
    let mut cmd = Command::new(exe);
    cmd.arg("hack");
    cmd
}

#[ext]
impl Command {
    fn test_dir(&mut self, path: &str) -> &mut Self {
        self.current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
    }

    fn assert_output(&mut self) -> AssertOutput {
        let output = self.output().unwrap_or_else(|e| panic!("could not execute process: {}", e));
        AssertOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            status: output.status,
        }
    }

    fn assert_success(&mut self) -> AssertOutput {
        let output = self.assert_output();
        if !output.status.success() {
            panic!(
                "assertion failed: `self.status.success()`:\n\nSTDOUT:\n{0}\n{1}\n{0}\n\nSTDERR:\n{0}\n{2}\n{0}\n",
                "-".repeat(60),
                output.stdout,
                output.stderr,
            )
        }
        output
    }

    fn assert_failure(&mut self) -> AssertOutput {
        let output = self.assert_output();
        if output.status.success() {
            panic!(
                "assertion failed: `!self.status.success()`:\n\nSTDOUT:\n{0}\n{1}\n{0}\n\nSTDERR:\n{0}\n{2}\n{0}\n",
                "-".repeat(60),
                output.stdout,
                output.stderr,
            )
        }
        output
    }
}

struct AssertOutput {
    stdout: String,
    stderr: String,
    status: ExitStatus,
}

fn line_separated(lines: &str, f: impl FnMut(&str)) {
    lines.split('\n').map(str::trim).filter(|line| !line.is_empty()).for_each(f);
}

impl AssertOutput {
    /// Receives a line(`\n`)-separated list of patterns and asserts whether stderr contains each pattern.
    fn assert_stderr_contains(&self, pats: &str) -> &Self {
        line_separated(pats, |pat| {
            if !self.stderr.contains(pat) {
                panic!(
                    "assertion failed: `self.stderr.contains(..)`:\n\nEXPECTED:\n{0}\n{1}\n{0}\n\nACTUAL:\n{0}\n{2}\n{0}\n",
                    "-".repeat(60),
                    pat,
                    self.stderr
                )
            }
        });
        self
    }

    /// Receives a line(`\n`)-separated list of patterns and asserts whether stdout contains each pattern.
    fn assert_stderr_not_contains(&self, pats: &str) -> &Self {
        line_separated(pats, |pat| {
            if self.stderr.contains(pat) {
                panic!(
                    "assertion failed: `!self.stderr.contains(..)`:\n\nEXPECTED:\n{0}\n{1}\n{0}\n\nACTUAL:\n{0}\n{2}\n{0}\n",
                    "-".repeat(60),
                    pat,
                    self.stderr
                )
            }
        });
        self
    }

    /// Receives a line(`\n`)-separated list of patterns and asserts whether stdout contains each pattern.
    fn assert_stdout_contains(&self, pats: &str) -> &Self {
        line_separated(pats, |pat| {
            if !self.stdout.contains(pat) {
                panic!(
                    "assertion failed: `self.stdout.contains(..)`:\n\nEXPECTED:\n{0}\n{1}\n{0}\n\nACTUAL:\n{0}\n{2}\n{0}\n",
                    "-".repeat(60),
                    pat,
                    self.stdout
                )
            }
        });
        self
    }

    /// Receives a line(`\n`)-separated list of patterns and asserts whether stdout contains each pattern.
    fn assert_stdout_not_contains(&self, pats: &str) -> &Self {
        line_separated(pats, |pat| {
            if self.stdout.contains(pat) {
                panic!(
                    "assertion failed: `!self.stdout.contains(..)`:\n\nEXPECTED:\n{0}\n{1}\n{0}\n\nACTUAL:\n{0}\n{2}\n{0}\n",
                    "-".repeat(60),
                    pat,
                    self.stdout
                )
            }
        });
        self
    }
}

#[test]
fn multi_arg() {
    // --package, -p, --exclude, --features, --exclude-features, and --verbose are allowed.

    for flag in &[
        "--examples",
        "--workspace",
        "--all",
        "--each-feature",
        "--feature-powerset",
        "--no-dev-deps",
        "--remove-dev-deps",
        "--ignore-private",
        "--ignore-unknown-features",
        "--optional-deps",
    ] {
        cargo_hack()
            .args(&["check", flag, flag])
            .test_dir("tests/fixtures/real")
            .assert_failure()
            .assert_stderr_contains(&format!(
                "The argument '{}' was provided more than once, but cannot be used multiple times",
                flag
            ));
    }

    for (flag, msg) in
        &[("--manifest-path", "--manifest-path <PATH>"), ("--color", "--color <WHEN>")]
    {
        cargo_hack()
            .args(&["check", flag, "auto", flag, "auto"])
            .test_dir("tests/fixtures/real")
            .assert_failure()
            .assert_stderr_contains(&format!(
                "The argument '{}' was provided more than once, but cannot be used multiple times",
                msg
            ));
    }
}

#[test]
fn removed_flags() {
    for (flag, alt) in &[("--ignore-non-exist-features", "--ignore-unknown-features")] {
        cargo_hack()
            .args(&["check", flag])
            .test_dir("tests/fixtures/real")
            .assert_failure()
            .assert_stderr_contains(&format!("{} was removed, use {} instead", flag, alt));
    }
}

#[test]
fn real_manifest() {
    cargo_hack()
        .args(&["check"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_not_contains(
            "running `cargo check` on member1
             running `cargo check` on member2
             running `cargo check` on member3",
        )
        .assert_stderr_contains("running `cargo check` on real");

    cargo_hack()
        .args(&["check", "--workspace"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1 (1/4)
             running `cargo check` on member2 (2/4)
             running `cargo check` on member3 (3/4)
             running `cargo check` on real (4/4)",
        );
}

#[test]
fn virtual_manifest() {
    cargo_hack()
        .args(&["check"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1 (1/3)
             running `cargo check` on member2 (2/3)",
        );

    cargo_hack()
        .args(&["check", "--all"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1 (1/3)
             running `cargo check` on member2 (2/3)",
        );
}

#[test]
fn real_all_in_subcrate() {
    cargo_hack()
        .args(&["check"])
        .test_dir("tests/fixtures/real/member2")
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member2")
        .assert_stderr_not_contains(
            "running `cargo check` on member1
             running `cargo check` on member3
             running `cargo check` on real",
        );

    cargo_hack()
        .args(&["check", "--all"])
        .test_dir("tests/fixtures/real/member2")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1
             running `cargo check` on member2
             running `cargo check` on member3
             running `cargo check` on real",
        );
}

#[test]
fn virtual_all_in_subcrate() {
    cargo_hack()
        .args(&["check"])
        .test_dir("tests/fixtures/virtual/member1")
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("running `cargo check` on member2");

    cargo_hack()
        .args(&["check", "--all"])
        .test_dir("tests/fixtures/virtual/member1")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1
             running `cargo check` on member2",
        );
}

#[test]
fn real_ignore_private() {
    cargo_hack()
        .args(&["check", "--ignore-private"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_not_contains(
            "running `cargo check` on member1
             skipped running on private crate member1
             running `cargo check` on member2
             skipped running on private crate member2
             running `cargo check` on real",
        )
        .assert_stderr_contains("skipped running on private crate real");

    cargo_hack()
        .args(&["check", "--all", "--ignore-private"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1
             skipped running on private crate member2
             running `cargo check` on member3
             skipped running on private crate real",
        )
        .assert_stderr_not_contains(
            "skipped running on private crate member1
             running `cargo check` on member2
             skipped running on private crate member3
             running `cargo check` on real",
        );
}

#[test]
fn virtual_ignore_private() {
    cargo_hack()
        .args(&["check", "--ignore-private"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1
             skipped running on private crate member2",
        )
        .assert_stderr_not_contains(
            "skipped running on private crate member1
             running `cargo check` on member2",
        );

    cargo_hack()
        .args(&["check", "--all", "--ignore-private"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1
             skipped running on private crate member2",
        )
        .assert_stderr_not_contains(
            "running `cargo check` on member2
             skipped running on private crate member1",
        );
}

#[test]
fn package() {
    cargo_hack()
        .args(&["check", "--package", "member1"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("running `cargo check` on member2");
}

#[test]
fn package_no_packages() {
    cargo_hack()
        .args(&["check", "--package", "foo"])
        .test_dir("tests/fixtures/virtual")
        .assert_failure()
        .assert_stderr_contains("package ID specification `foo` matched no packages");
}

#[test]
fn exclude() {
    cargo_hack()
        .args(&["check", "--all", "--exclude", "member1"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_not_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2");

    // not_found is warning
    cargo_hack()
        .args(&["check", "--all", "--exclude", "foo"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(
            "excluded package(s) foo not found in workspace
             running `cargo check` on member1
             running `cargo check` on member2",
        );
}

#[test]
fn exclude_failure() {
    // not with --workspace
    cargo_hack()
        .args(&["check", "--exclude", "member1"])
        .test_dir("tests/fixtures/virtual")
        .assert_failure()
        .assert_stderr_contains("--exclude can only be used together with --workspace");
}

#[test]
fn no_dev_deps() {
    cargo_hack()
        .args(&["check", "--no-dev-deps"])
        .test_dir("tests/fixtures/no_dev_deps")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on no_dev_deps
             --no-dev-deps removes dev-dependencies from real `Cargo.toml` while cargo-hack is \
             running and restores it when finished",
        );

    // with --all
    cargo_hack()
        .args(&["check", "--no-dev-deps", "--all"])
        .test_dir("tests/fixtures/no_dev_deps")
        .assert_success()
        .assert_stderr_contains(
            "--no-dev-deps removes dev-dependencies from real `Cargo.toml` while cargo-hack is running and restores it when finished",
        );
}

#[test]
fn no_dev_deps_failure() {
    // with --remove-dev-deps
    cargo_hack()
        .args(&["check", "--no-dev-deps", "--remove-dev-deps"])
        .test_dir("tests/fixtures/no_dev_deps")
        .assert_failure()
        .assert_stderr_contains("--no-dev-deps may not be used together with --remove-dev-deps");

    // with options requires dev-deps
    for flag in
        &["--example", "--examples", "--test", "--tests", "--bench", "--benches", "--all-targets"]
    {
        cargo_hack()
            .args(&["check", "--no-dev-deps", flag])
            .test_dir("tests/fixtures/real")
            .assert_failure()
            .assert_stderr_contains(&format!(
                "--no-dev-deps may not be used together with {}",
                flag
            ));
    }

    // with subcommands requires dev-deps
    for subcommand in &["test", "bench"] {
        cargo_hack()
            .args(&[subcommand, "--no-dev-deps"])
            .test_dir("tests/fixtures/real")
            .assert_failure()
            .assert_stderr_contains(&format!(
                "--no-dev-deps may not be used together with {} subcommand",
                subcommand
            ));
    }
}

#[test]
fn remove_dev_deps_failure() {
    // with options requires dev-deps
    for flag in
        &["--example", "--examples", "--test", "--tests", "--bench", "--benches", "--all-targets"]
    {
        cargo_hack()
            .args(&["check", "--remove-dev-deps", flag])
            .test_dir("tests/fixtures/real")
            .assert_failure()
            .assert_stderr_contains(&format!(
                "--remove-dev-deps may not be used together with {}",
                flag
            ));
    }

    // with subcommands requires dev-deps
    for subcommand in &["test", "bench"] {
        cargo_hack()
            .args(&[subcommand, "--remove-dev-deps"])
            .test_dir("tests/fixtures/real")
            .assert_failure()
            .assert_stderr_contains(&format!(
                "--remove-dev-deps may not be used together with {} subcommand",
                subcommand
            ));
    }
}

#[test]
fn ignore_unknown_features() {
    cargo_hack()
        .args(&["check", "--ignore-unknown-features", "--no-default-features", "--features", "f"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(
            "skipped applying unknown `f` feature to member1
             running `cargo check --no-default-features` on member1
             running `cargo check --no-default-features --features f` on member2",
        )
        .assert_stderr_not_contains("skipped applying unknown `f` feature to member2");
}

#[test]
fn ignore_unknown_features_failure() {
    cargo_hack()
        .args(&["check", "--ignore-unknown-features"])
        .test_dir("tests/fixtures/virtual")
        .assert_failure()
        .assert_stderr_contains(
            "--ignore-unknown-features can only be used together with either --features or --include-features",
        );
}

#[test]
fn each_feature() {
    cargo_hack()
        .args(&["check", "--each-feature"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on real (1/6)
             running `cargo check --no-default-features` on real (2/6)
             running `cargo check --no-default-features --features a` on real (3/6)
             running `cargo check --no-default-features --features b` on real (4/6)
             running `cargo check --no-default-features --features c` on real (5/6)
             running `cargo check --no-default-features --all-features` on real (6/6)",
        );

    // with other feature
    cargo_hack()
        .args(&["check", "--each-feature", "--features", "a"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check --features a` on real (1/5)
             running `cargo check --no-default-features --features a` on real (2/5)
             running `cargo check --no-default-features --features a,b` on real (3/5)
             running `cargo check --no-default-features --features a,c` on real (4/5)
             running `cargo check --no-default-features --all-features --features a` on real (5/5)",
        )
        .assert_stderr_not_contains("--features a,a");
}

#[test]
fn feature_powerset() {
    cargo_hack()
        .args(&["check", "--feature-powerset"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on real (1/10)
             running `cargo check --no-default-features` on real (2/10)
             running `cargo check --no-default-features --features a` on real (3/10)
             running `cargo check --no-default-features --features b` on real (4/10)
             running `cargo check --no-default-features --features c` on real (6/10)
             running `cargo check --no-default-features --features a,b` on real (5/10)
             running `cargo check --no-default-features --features a,c` on real (7/10)
             running `cargo check --no-default-features --features b,c` on real (8/10)
             running `cargo check --no-default-features --features a,b,c` on real (9/10)
             running `cargo check --no-default-features --all-features` on real (10/10)",
        );

    // with other feature
    cargo_hack()
        .args(&["check", "--feature-powerset", "--features", "a"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check --features a` on real (1/6)
             running `cargo check --no-default-features --features a` on real (2/6)
             running `cargo check --no-default-features --features a,b` on real (3/6)
             running `cargo check --no-default-features --features a,c` on real (4/6)
             running `cargo check --no-default-features --features a,b,c` on real (5/6)
             running `cargo check --no-default-features --all-features --features a` on real (6/6)",
        )
        .assert_stderr_not_contains("--features a,a");
}

#[test]
fn feature_powerset_depth() {
    cargo_hack()
        .args(&["check", "--feature-powerset", "--depth", "2"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on real (1/9)
             running `cargo check --no-default-features` on real (2/9)
             running `cargo check --no-default-features --features a` on real (3/9)
             running `cargo check --no-default-features --features b` on real (4/9)
             running `cargo check --no-default-features --features c` on real (6/9)
             running `cargo check --no-default-features --features a,b` on real (5/9)
             running `cargo check --no-default-features --features a,c` on real (7/9)
             running `cargo check --no-default-features --features b,c` on real (8/9)
             running `cargo check --no-default-features --all-features` on real (9/9)",
        )
        .assert_stderr_not_contains("--features a,b,c");
}

#[test]
fn depth_failure() {
    cargo_hack()
        .args(&["check", "--each-feature", "--depth", "2"])
        .test_dir("tests/fixtures/real")
        .assert_failure()
        .assert_stderr_contains("--depth can only be used together with --feature-powerset");
}

#[test]
fn include_features() {
    cargo_hack()
        .args(&["check", "--each-feature", "--include-features", "a,b"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features a` on real (1/2)
             running `cargo check --no-default-features --features b` on real (2/2)",
        )
        .assert_stderr_not_contains("--features c");

    cargo_hack()
        .args(&["check", "--feature-powerset", "--include-features", "a,b"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features a` on real (1/3)
             running `cargo check --no-default-features --features b` on real (2/3)
             running `cargo check --no-default-features --features a,b` on real (3/3)",
        );
}

#[test]
fn exclude_features_failure() {
    cargo_hack()
        .args(&["check", "--exclude-features", "a"])
        .test_dir("tests/fixtures/real")
        .assert_failure()
        .assert_stderr_contains(
            "--exclude-features (--skip) can only be used together with either --each-feature or --feature-powerset",
        );
}

#[test]
fn each_feature_skip_success() {
    cargo_hack()
        .args(&["check", "--each-feature", "--exclude-features", "a"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on real (1/5)
             running `cargo check --no-default-features` on real (2/5)
             running `cargo check --no-default-features --features b` on real (3/5)
             running `cargo check --no-default-features --features c` on real (4/5)
             running `cargo check --no-default-features --all-features` on real (5/5)",
        )
        .assert_stderr_not_contains("--features a");

    cargo_hack()
        .args(&["check", "--each-feature", "--exclude-features", "a b"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on real (1/4)
             running `cargo check --no-default-features` on real (2/4)
             running `cargo check --no-default-features --features c` on real (3/4)
             running `cargo check --no-default-features --all-features` on real (4/4)",
        )
        .assert_stderr_not_contains(
            "--features a
             --features b",
        );

    cargo_hack()
        .args(&["check", "--each-feature", "--exclude-features", "a", "--exclude-features", "b"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on real (1/4)
             running `cargo check --no-default-features` on real (2/4)
             running `cargo check --no-default-features --features c` on real (3/4)
             running `cargo check --no-default-features --all-features` on real (4/4)",
        )
        .assert_stderr_not_contains(
            "--features a
             --features b",
        );
}

#[test]
fn powerset_skip_success() {
    cargo_hack()
        .args(&["check", "--feature-powerset", "--exclude-features", "a"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on real (1/6)
             running `cargo check --no-default-features` on real (2/6)
             running `cargo check --no-default-features --features b` on real (3/6)
             running `cargo check --no-default-features --features c` on real (4/6)
             running `cargo check --no-default-features --features b,c` on real (5/6)
             running `cargo check --no-default-features --all-features` on real (6/6)",
        )
        .assert_stderr_not_contains(
            "--features a
             --features a,b
             --features a,c
             --features a,b,c",
        );
}

#[test]
fn exclude_features_default() {
    cargo_hack()
        .args(&["check", "--each-feature", "--exclude-features", "default"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_not_contains("running `cargo check` on real")
        .assert_stderr_contains(
            "running `cargo check --no-default-features` on real (1/5)
             running `cargo check --no-default-features --features a` on real (2/5)
             running `cargo check --no-default-features --features b` on real (3/5)
             running `cargo check --no-default-features --features c` on real (4/5)
             running `cargo check --no-default-features --all-features` on real (5/5)",
        );
}

#[test]
fn exclude_no_default_features() {
    cargo_hack()
        .args(&["check", "--each-feature", "--exclude-no-default-features"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on real (1/5)
             running `cargo check --no-default-features --features a` on real (2/5)
             running `cargo check --no-default-features --features b` on real (3/5)
             running `cargo check --no-default-features --features c` on real (4/5)
             running `cargo check --no-default-features --all-features` on real (5/5)",
        )
        .assert_stderr_not_contains("running `cargo check --no-default-features` on real");

    // --skip-no-default-features is a deprecated alias of --exclude-no-default-features
    cargo_hack()
        .args(&["check", "--each-feature", "--skip-no-default-features"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(
            "--skip-no-default-features is deprecated, use --exclude-no-default-features flag instead",
        );
}

#[test]
fn exclude_no_default_features_failure() {
    cargo_hack()
        .args(&["check", "--exclude-no-default-features"])
        .test_dir("tests/fixtures/real")
        .assert_failure()
        .assert_stderr_contains(
            "--exclude-no-default-features can only be used together with either --each-feature or --feature-powerset",
        );
}

#[test]
fn exclude_all_features() {
    cargo_hack()
        .args(&["check", "--each-feature", "--exclude-all-features"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on real (1/5)
             running `cargo check --no-default-features` on real (2/5)
             running `cargo check --no-default-features --features a` on real (3/5)
             running `cargo check --no-default-features --features b` on real (4/5)
             running `cargo check --no-default-features --features c` on real (5/5)",
        )
        .assert_stderr_not_contains(
            "running `cargo check --no-default-features --all-features` on real",
        );
}

#[test]
fn exclude_all_features_failure() {
    cargo_hack()
        .args(&["check", "--exclude-all-features"])
        .test_dir("tests/fixtures/real")
        .assert_failure()
        .assert_stderr_contains(
            "--exclude-all-features can only be used together with either --each-feature or --feature-powerset",
        );
}

#[test]
fn each_feature_all() {
    cargo_hack()
        .args(&["check", "--each-feature", "--workspace"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1 (1/24)
             running `cargo check --no-default-features` on member1 (2/24)
             running `cargo check --no-default-features --features a` on member1 (3/24)
             running `cargo check --no-default-features --features b` on member1 (4/24)
             running `cargo check --no-default-features --features c` on member1 (5/24)
             running `cargo check --no-default-features --all-features` on member1 (6/24)
             running `cargo check` on member2 (7/24)
             running `cargo check --no-default-features` on member2 (8/24)
             running `cargo check --no-default-features --features a` on member2 (9/24)
             running `cargo check --no-default-features --features b` on member2 (10/24)
             running `cargo check --no-default-features --features c` on member2 (11/24)
             running `cargo check --no-default-features --all-features` on member2 (12/24)
             running `cargo check` on member3 (13/24)
             running `cargo check --no-default-features` on member3 (14/24)
             running `cargo check --no-default-features --features a` on member3 (15/24)
             running `cargo check --no-default-features --features b` on member3 (16/24)
             running `cargo check --no-default-features --features c` on member3 (17/24)
             running `cargo check --no-default-features --all-features` on member3 (18/24)
             running `cargo check` on real (19/24)
             running `cargo check --no-default-features` on real (20/24)
             running `cargo check --no-default-features --features a` on real (21/24)
             running `cargo check --no-default-features --features b` on real (22/24)
             running `cargo check --no-default-features --features c` on real (23/24)
             running `cargo check --no-default-features --all-features` on real (24/24)",
        );
}

#[rustversion::attr(not(since(1.41)), ignore)]
#[test]
fn include_deps_features() {
    cargo_hack()
        .args(&["check", "--each-feature", "--include-deps-features"])
        .test_dir("tests")
        .assert_success()
        .assert_stderr_contains("
            running `cargo check` on cargo-hack (1/21)
            running `cargo check --no-default-features` on cargo-hack (2/21)
            running `cargo check --no-default-features --features anyhow/default` on cargo-hack (3/21)
            running `cargo check --no-default-features --features anyhow/std` on cargo-hack (4/21)
            running `cargo check --no-default-features --features ctrlc/termination` on cargo-hack (5/21)
            running `cargo check --no-default-features --features serde_json/alloc` on cargo-hack (6/21)
            running `cargo check --no-default-features --features serde_json/arbitrary_precision` on cargo-hack (7/21)
            running `cargo check --no-default-features --features serde_json/default` on cargo-hack (8/21)
            running `cargo check --no-default-features --features serde_json/float_roundtrip` on cargo-hack (9/21)
            running `cargo check --no-default-features --features serde_json/preserve_order` on cargo-hack (10/21)
            running `cargo check --no-default-features --features serde_json/raw_value` on cargo-hack (11/21)
            running `cargo check --no-default-features --features serde_json/std` on cargo-hack (12/21)
            running `cargo check --no-default-features --features serde_json/unbounded_depth` on cargo-hack (13/21)
            running `cargo check --no-default-features --features term_size/debug` on cargo-hack (14/21)
            running `cargo check --no-default-features --features term_size/default` on cargo-hack (15/21)
            running `cargo check --no-default-features --features term_size/nightly` on cargo-hack (16/21)
            running `cargo check --no-default-features --features term_size/travis` on cargo-hack (17/21)
            running `cargo check --no-default-features --features term_size/unstable` on cargo-hack (18/21)
            running `cargo check --no-default-features --features toml/default` on cargo-hack (19/21)
            running `cargo check --no-default-features --features toml/preserve_order` on cargo-hack (20/21)
            running `cargo check --no-default-features --all-features` on cargo-hack (21/21)
        ");
}

#[rustversion::attr(not(before(1.41)), ignore)]
#[test]
fn include_deps_features_version_failure() {
    cargo_hack()
        .args(&["check", "--each-feature", "--include-deps-features"])
        .test_dir("tests")
        .assert_failure()
        .assert_stderr_contains("--all-features requires Cargo 1.41 or leter");
}

#[test]
fn trailing_args() {
    cargo_hack()
        .args(&["test", "--", "--ignored"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains("running `cargo test -- --ignored` on real")
        .assert_stdout_contains(
            "running 1 test
             test tests::test_ignored",
        );
}

#[test]
fn package_collision() {
    cargo_hack()
        .args(&["check"])
        .test_dir("tests/fixtures/package_collision")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1
             running `cargo check` on member2",
        );
}

#[test]
fn not_find_manifest() {
    cargo_hack()
        .args(&["check"])
        .test_dir("tests/fixtures/virtual/dir/not_find_manifest")
        .assert_success()
        .assert_stderr_not_contains(
            "running `cargo check` on member1
             running `cargo check` on member2",
        )
        .assert_stderr_contains("running `cargo check` on not_find_manifest");

    cargo_hack()
        .args(&["check", "--all"])
        .test_dir("tests/fixtures/virtual/dir/not_find_manifest")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1
             running `cargo check` on member2
             running `cargo check` on not_find_manifest",
        );

    cargo_hack()
        .args(&["check", "--manifest-path", "dir/not_find_manifest/Cargo.toml"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_not_contains(
            "running `cargo check` on member1
             running `cargo check` on member2",
        )
        .assert_stderr_contains("running `cargo check` on not_find_manifest");

    cargo_hack()
        .args(&["check", "--all", "--manifest-path", "dir/not_find_manifest/Cargo.toml"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1
             running `cargo check` on member2
             running `cargo check` on not_find_manifest",
        );
}

#[test]
fn optional_deps() {
    cargo_hack()
        .args(&["run", "--features=real,member2,renemed", "--ignore-unknown-features"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains(
            "skipped applying unknown `member2` feature to optional_deps
             running `cargo run --features real,renemed` on optional_deps",
        )
        .assert_stdout_contains(
            "renemed
             real",
        )
        .assert_stdout_not_contains(
            "member3
             member2",
        );

    cargo_hack()
        .args(&["check", "--each-feature"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo check` on optional_deps (1/1)")
        .assert_stderr_not_contains(
            "--no-default-features
             --features real
             --features renemed",
        );

    cargo_hack()
        .args(&["check", "--each-feature", "--optional-deps"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on optional_deps (1/5)
             running `cargo check --no-default-features` on optional_deps (2/5)
             running `cargo check --no-default-features --features real` on optional_deps (3/5)
             running `cargo check --no-default-features --features renemed` on optional_deps (4/5)
             running `cargo check --no-default-features --all-features` on optional_deps (5/5)",
        );

    cargo_hack()
        .args(&["check", "--each-feature", "--optional-deps", "real"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on optional_deps (1/4)
             running `cargo check --no-default-features` on optional_deps (2/4)
             running `cargo check --no-default-features --features real` on optional_deps (3/4)
             running `cargo check --no-default-features --all-features` on optional_deps (4/4)",
        )
        .assert_stderr_not_contains("--features renemed");

    cargo_hack()
        .args(&["check", "--each-feature", "--optional-deps=renemed"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on optional_deps (1/4)
             running `cargo check --no-default-features` on optional_deps (2/4)
             running `cargo check --no-default-features --features renemed` on optional_deps (3/4)
             running `cargo check --no-default-features --all-features` on optional_deps (4/4)",
        )
        .assert_stderr_not_contains("--features real");

    cargo_hack()
        .args(&["check", "--each-feature", "--optional-deps="])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo check` on optional_deps (1/1)");
}

#[test]
fn optional_deps_failure() {
    cargo_hack()
        .args(&["check", "--optional-deps"])
        .test_dir("tests/fixtures/real")
        .assert_failure()
        .assert_stderr_contains(
            "--optional-deps can only be used together with either --each-feature or --feature-powerset",
        );
}

#[test]
fn skip_optional_deps() {
    cargo_hack()
        .args(&["check", "--each-feature", "--optional-deps", "--exclude-features", "real"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on optional_deps (1/4)
             running `cargo check --no-default-features` on optional_deps (2/4)
             running `cargo check --no-default-features --features renemed` on optional_deps (3/4)
             running `cargo check --no-default-features --all-features` on optional_deps (4/4)",
        )
        .assert_stderr_not_contains("--features real");
}

#[test]
fn list_separator() {
    cargo_hack()
        .args(&["run", "--features='real,renemed'"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack()
        .args(&["run", "--features=\"real,renemed\""])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack()
        .args(&["run", "--features=real,renemed"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack()
        .args(&["run", "--features", "real,renemed"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack()
        .args(&["run", "--features='real renemed'"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack()
        .args(&["run", "--features=\"real renemed\""])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack()
        .args(&["run", "--features", "real renemed"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo run --features real,renemed` on optional_deps");
}

#[test]
fn verbose() {
    cargo_hack()
        .args(&["check", "--verbose"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(&format!(
            "running `cargo check --manifest-path member1{0}Cargo.toml`
             running `cargo check --manifest-path member2{0}Cargo.toml`
             running `cargo check --manifest-path dir{0}not_find_manifest{0}Cargo.toml`",
            SEPARATOR
        ));
}

#[test]
fn propagate() {
    // --features
    cargo_hack()
        .args(&["check", "--features", "a"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains("--features a");
    cargo_hack()
        .args(&["check", "--features=a"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains("--features a");

    // --no-default-features
    cargo_hack()
        .args(&["check", "--no-default-features"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains("--no-default-features");

    // --all-features
    cargo_hack()
        .args(&["check", "--all-features"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains("--all-features");

    // --color
    cargo_hack()
        .args(&["check", "--color", "auto"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains("`cargo check --color auto`");
    cargo_hack()
        .args(&["check", "--color=auto"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains("`cargo check --color=auto`");

    // --verbose does not be propagated
    cargo_hack()
        .args(&["check", "--verbose"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_not_contains("--verbose");
}
