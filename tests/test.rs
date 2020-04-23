use std::{
    borrow::Cow,
    env,
    path::PathBuf,
    process::{Command, Output},
};

fn cargo_hack() -> Command {
    let mut current = env::current_exe().unwrap();
    current.pop();
    if current.ends_with("deps") {
        current.pop();
    }
    let mut cmd = Command::new(current.join("cargo-hack"));
    cmd.arg("hack");
    cmd
}

fn test_dir(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

#[easy_ext::ext]
impl Output {
    fn stdout(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.stdout)
    }
    fn stderr(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.stderr)
    }
    fn assert_success(&self) -> &Self {
        if !self.status.success() {
            panic!(
                "`self.status.success()`:\n\nSTDOUT:\n```\n{}\n```\n\nSTDERR:\n```\n{}\n```\n",
                self.stdout(),
                self.stderr(),
            )
        }
        self
    }
    fn assert_failure(&self) -> &Self {
        if self.status.success() {
            panic!(
                "`!self.status.success()`:\n\nSTDOUT:\n```\n{}\n```\n\nSTDERR:\n```\n{}\n```\n",
                self.stdout(),
                self.stderr(),
            )
        }
        self
    }
    fn assert_stderr_contains(&self, pat: &str) -> &Self {
        if !self.stderr().contains(pat) {
            panic!(
                "`self.stderr().contains(..)`:\n\nEXPECTED:\n```\n{}\n```\n\nACTUAL:\n```\n{}\n```\n",
                pat,
                self.stderr()
            )
        }
        self
    }
    fn assert_stderr_not_contains(&self, pat: &str) -> &Self {
        if self.stderr().contains(pat) {
            panic!(
                "`!self.stderr().contains(..)`:\n\nUNEXPECTED:\n```\n{}\n```\n\nACTUAL:\n```\n{}\n```\n",
                pat,
                self.stderr()
            )
        }
        self
    }
    fn assert_stdout_contains(&self, pat: &str) -> &Self {
        if !self.stdout().contains(pat) {
            panic!(
                "`self.stdout().contains(..)`:\n\nEXPECTED:\n```\n{}\n```\n\nACTUAL:\n```\n{}\n```\n",
                pat,
                self.stdout()
            )
        }
        self
    }
    fn assert_stdout_not_contains(&self, pat: &str) -> &Self {
        if self.stdout().contains(pat) {
            panic!(
                "`!self.stdout().contains(..)`:\n\nUNEXPECTED:\n```\n{}\n```\n\nACTUAL:\n```\n{}\n```\n",
                pat,
                self.stdout()
            )
        }
        self
    }
}

#[test]
fn test_real() {
    cargo_hack()
        .args(&["check"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_not_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("running `cargo check` on member2")
        .assert_stderr_not_contains("running `cargo check` on member3")
        .assert_stderr_contains("running `cargo check` on real");

    cargo_hack()
        .args(&["check", "--all"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2")
        .assert_stderr_contains("running `cargo check` on member3")
        .assert_stderr_contains("running `cargo check` on real");
}

#[test]
fn test_virtual() {
    cargo_hack()
        .args(&["check"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2");

    cargo_hack()
        .args(&["check", "--all"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2");
}

#[test]
fn test_real_all_in_subcrate() {
    cargo_hack()
        .args(&["check"])
        .current_dir(test_dir("tests/fixtures/real/member2"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_not_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2")
        .assert_stderr_not_contains("running `cargo check` on member3")
        .assert_stderr_not_contains("running `cargo check` on real");

    cargo_hack()
        .args(&["check", "--all"])
        .current_dir(test_dir("tests/fixtures/real/member2"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2")
        .assert_stderr_contains("running `cargo check` on member3")
        .assert_stderr_contains("running `cargo check` on real");
}

#[test]
fn test_virtual_all_in_subcrate() {
    cargo_hack()
        .args(&["check"])
        .current_dir(test_dir("tests/fixtures/virtual/member1"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("running `cargo check` on member2");

    cargo_hack()
        .args(&["check", "--all"])
        .current_dir(test_dir("tests/fixtures/virtual/member1"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2");
}

#[test]
fn test_real_ignore_private() {
    cargo_hack()
        .args(&["check", "--ignore-private"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_not_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("skipped running on member1")
        .assert_stderr_not_contains("running `cargo check` on member2")
        .assert_stderr_not_contains("skipped running on member2")
        .assert_stderr_not_contains("running `cargo check` on real")
        .assert_stderr_contains("skipped running on real");

    cargo_hack()
        .args(&["check", "--all", "--ignore-private"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("skipped running on member1")
        .assert_stderr_not_contains("running `cargo check` on member2")
        .assert_stderr_contains("skipped running on member2")
        .assert_stderr_contains("running `cargo check` on member3")
        .assert_stderr_not_contains("skipped running on member3")
        .assert_stderr_not_contains("running `cargo check` on real")
        .assert_stderr_contains("skipped running on real");
}

#[test]
fn test_virtual_ignore_private() {
    cargo_hack()
        .args(&["check", "--ignore-private"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("running `cargo check` on member2")
        .assert_stderr_not_contains("skipped running on member1")
        .assert_stderr_contains("skipped running on member2");

    cargo_hack()
        .args(&["check", "--all", "--ignore-private"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("running `cargo check` on member2")
        .assert_stderr_not_contains("skipped running on member1")
        .assert_stderr_contains("skipped running on member2");
}

#[test]
fn test_package() {
    cargo_hack()
        .args(&["check", "--package", "member1"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("running `cargo check` on member2");
}

#[test]
fn test_package_no_packages() {
    cargo_hack()
        .args(&["check", "--package", "foo"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap()
        .assert_failure()
        .assert_stderr_contains("package ID specification `foo` matched no packages");
}

#[test]
fn test_exclude() {
    cargo_hack()
        .args(&["check", "--all", "--exclude", "foo"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("excluded package(s) foo not found in workspace")
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2");
}

#[test]
fn test_exclude_not_found() {
    cargo_hack()
        .args(&["check", "--all", "--exclude", "member1"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_not_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2");
}

#[test]
fn test_exclude_not_with_all() {
    cargo_hack()
        .args(&["check", "--exclude", "member1"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap()
        .assert_failure()
        .assert_stderr_contains("--exclude can only be used together with --workspace");
}

#[test]
fn test_remove_dev_deps_with_devs() {
    for flag in &[
        "--example",
        "--examples",
        "--test",
        "--tests",
        "--bench",
        "--benches",
        "--all-targets",
    ][..]
    {
        cargo_hack()
            .args(&["check", "--remove-dev-deps", flag])
            .current_dir(test_dir("tests/fixtures/real"))
            .output()
            .unwrap()
            .assert_failure()
            .assert_stderr_contains(&format!(
                "--remove-dev-deps may not be used together with {}",
                flag
            ));
    }

    for subcommand in &["test", "bench"] {
        cargo_hack()
            .args(&[subcommand, "--remove-dev-deps"])
            .current_dir(test_dir("tests/fixtures/real"))
            .output()
            .unwrap()
            .assert_failure()
            .assert_stderr_contains(&format!(
                "--remove-dev-deps may not be used together with {} subcommand",
                subcommand
            ));
    }
}

#[test]
fn test_no_dev_deps() {
    cargo_hack()
        .args(&["check", "--no-dev-deps", "--remove-dev-deps"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap()
        .assert_failure()
        .assert_stderr_contains("--no-dev-deps may not be used together with --remove-dev-deps");

    cargo_hack()
        .args(&["check", "--no-dev-deps"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on real")
        .assert_stderr_contains(
            "`--no-dev-deps` flag removes dev-dependencies from real `Cargo.toml` while cargo-hack is running and restores it when finished",
        );
}

#[test]
fn test_no_dev_deps_all() {
    cargo_hack()
        .args(&["check", "--no-dev-deps", "--all"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains(
            "`--no-dev-deps` flag removes dev-dependencies from real `Cargo.toml` while cargo-hack is running and restores it when finished",
        );
}

#[test]
fn test_no_dev_deps_with_devs() {
    for flag in &[
        "--example",
        "--examples",
        "--test",
        "--tests",
        "--bench",
        "--benches",
        "--all-targets",
    ][..]
    {
        cargo_hack()
            .args(&["check", "--no-dev-deps", flag])
            .current_dir(test_dir("tests/fixtures/real"))
            .output()
            .unwrap()
            .assert_failure()
            .assert_stderr_contains(&format!(
                "--no-dev-deps may not be used together with {}",
                flag
            ));
    }

    for subcommand in &["test", "bench"] {
        cargo_hack()
            .args(&[subcommand, "--no-dev-deps"])
            .current_dir(test_dir("tests/fixtures/real"))
            .output()
            .unwrap()
            .assert_failure()
            .assert_stderr_contains(&format!(
                "--no-dev-deps may not be used together with {} subcommand",
                subcommand
            ));
    }
}

#[test]
fn test_ignore_unknown_features() {
    cargo_hack()
        .args(&["check", "--ignore-unknown-features", "--no-default-features", "--features", "f"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("skipped applying unknown `f` feature to member1")
        .assert_stderr_contains("running `cargo check --no-default-features` on member1")
        .assert_stderr_not_contains("skipped applying unknown `f` feature to member2")
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features f` on member2",
        );
}

#[test]
fn test_ignore_non_exist_features() {
    cargo_hack()
        .args(&["check", "--ignore-non-exist-features", "--features=f"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("'--ignore-non-exist-features' flag is deprecated, use '--ignore-unknown-features' flag instead")
        .assert_stderr_contains("skipped applying unknown `f` feature to member1")
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("skipped applying unknown `f` feature to member2")
        .assert_stderr_contains("running `cargo check --features f` on member2");
}

#[test]
fn test_each_feature() {
    cargo_hack()
        .args(&["check", "--each-feature"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on real (1/5)")
        .assert_stderr_contains("running `cargo check --no-default-features` on real (2/5)")
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features a` on real (3/5)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features b` on real (4/5)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features c` on real (5/5)",
        );
}

#[test]
fn test_feature_powerset() {
    cargo_hack()
        .args(&["check", "--feature-powerset"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on real (1/9)")
        .assert_stderr_contains("running `cargo check --no-default-features` on real (2/9)")
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features a` on real (3/9)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features b` on real (4/9)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features c` on real (6/9)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features a,b` on real (5/9)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features a,c` on real (7/9)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features b,c` on real (8/9)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features a,b,c` on real (9/9)",
        );
}

#[test]
fn test_skip_failure() {
    cargo_hack()
        .args(&["check", "--skip", "a"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap()
        .assert_failure()
        .assert_stderr_contains(
            "--skip can only be used with either --each-feature or --feature-powerset",
        );
}

#[test]
fn test_each_feature_skip_success() {
    cargo_hack()
        .args(&["check", "--each-feature", "--skip", "a"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on real (1/4)")
        .assert_stderr_contains("running `cargo check --no-default-features` on real (2/4)")
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features b` on real (3/4)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features c` on real (4/4)",
        )
        .assert_stderr_not_contains(
            "running `cargo check --no-default-features --features a` on real",
        );
}

#[test]
fn test_powerset_skip_success() {
    cargo_hack()
        .args(&["check", "--feature-powerset", "--skip", "a"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on real")
        .assert_stderr_contains("running `cargo check --no-default-features` on real")
        .assert_stderr_contains("running `cargo check --no-default-features --features b` on real")
        .assert_stderr_contains("running `cargo check --no-default-features --features c` on real")
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features b,c` on real",
        )
        .assert_stderr_not_contains(
            "running `cargo check --no-default-features --features a` on real",
        )
        .assert_stderr_not_contains(
            "running `cargo check --no-default-features --features a,b` on real",
        )
        .assert_stderr_not_contains(
            "running `cargo check --no-default-features --features a,c` on real",
        )
        .assert_stderr_not_contains(
            "running `cargo check --no-default-features --features a,b,c` on real",
        );
}

#[test]
fn test_each_feature2() {
    cargo_hack()
        .args(&["check", "--each-feature", "--features=a"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check --features a` on real (1/5)")
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features a` on real (2/5)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features a,a` on real (3/5)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features a,b` on real (4/5)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features a,c` on real (5/5)",
        );
}

#[test]
fn test_each_feature_all() {
    cargo_hack()
        .args(&["check", "--each-feature", "--workspace"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1 (1/20)")
        .assert_stderr_contains("running `cargo check --no-default-features` on member1 (2/20)")
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features a` on member1 (3/20)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features b` on member1 (4/20)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features c` on member1 (5/20)",
        )
        .assert_stderr_contains("running `cargo check` on member2 (6/20)")
        .assert_stderr_contains("running `cargo check --no-default-features` on member2 (7/20)")
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features a` on member2 (8/20)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features b` on member2 (9/20)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features c` on member2 (10/20)",
        )
        .assert_stderr_contains("running `cargo check` on member3 (11/20)")
        .assert_stderr_contains("running `cargo check --no-default-features` on member3 (12/20)")
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features a` on member3 (13/20)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features b` on member3 (14/20)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features c` on member3 (15/20)",
        )
        .assert_stderr_contains("running `cargo check` on real (16/20)")
        .assert_stderr_contains("running `cargo check --no-default-features` on real (17/20)")
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features a` on real (18/20)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features b` on real (19/20)",
        )
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features c` on real (20/20)",
        );
}

#[test]
fn test_args2() {
    cargo_hack()
        .args(&["test", "--", "--ignored"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo test -- --ignored` on real")
        .assert_stdout_contains("running 1 test")
        .assert_stdout_contains("test tests::test_ignored");
}

#[test]
fn test_package_collision() {
    cargo_hack()
        .args(&["check"])
        .current_dir(test_dir("tests/fixtures/package_collision"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2");
}

#[test]
fn test_not_find_manifest() {
    cargo_hack()
        .args(&["check"])
        .current_dir(test_dir("tests/fixtures/virtual/dir/not_find_manifest"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_not_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("running `cargo check` on member2")
        .assert_stderr_contains("running `cargo check` on not_find_manifest");

    cargo_hack()
        .args(&["check", "--all"])
        .current_dir(test_dir("tests/fixtures/virtual/dir/not_find_manifest"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2")
        .assert_stderr_contains("running `cargo check` on not_find_manifest");

    cargo_hack()
        .args(&["check", "--manifest-path", "dir/not_find_manifest/Cargo.toml"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_not_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("running `cargo check` on member2")
        .assert_stderr_contains("running `cargo check` on not_find_manifest");

    cargo_hack()
        .args(&["check", "--all", "--manifest-path", "dir/not_find_manifest/Cargo.toml"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap()
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2")
        .assert_stderr_contains("running `cargo check` on not_find_manifest");
}
