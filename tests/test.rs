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
    Command::new(current.join("cargo-hack"))
}

fn test_dir(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

#[easy_ext::ext(OutputExt)]
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
                "`self.status.success()`:\nSTDOUT:\n```\n{}\n```\n\nSTDERR:\n```\n{}\n```\n",
                self.stdout(),
                self.stderr(),
            )
        }
        self
    }
    fn assert_failure(&self) -> &Self {
        if self.status.success() {
            panic!(
                "`!self.status.success()`:\nSTDOUT:\n```\n{}\n```\n\nSTDERR:\n```\n{}\n```\n",
                self.stdout(),
                self.stderr(),
            )
        }
        self
    }
    fn assert_stderr_contains(&self, pat: &str) -> &Self {
        if !self.stderr().contains(pat) {
            panic!(
                "`self.stderr().contains(pat)`:\nPAT:\n```\n{}\n```\n\nACTUAL:\n```\n{}\n```\n",
                pat,
                self.stderr()
            )
        }
        self
    }
    fn assert_stderr_not_contains(&self, pat: &str) -> &Self {
        if self.stderr().contains(pat) {
            panic!(
                "`!self.stderr().contains(pat)`:\nPAT:\n```\n{}\n```\n\nACTUAL:\n```\n{}\n```\n",
                pat,
                self.stderr()
            )
        }
        self
    }
    fn assert_stdout_contains(&self, pat: &str) -> &Self {
        if !self.stdout().contains(pat) {
            panic!(
                "`self.stdout().contains(pat)`:\nPAT:\n```\n{}\n```\n\nACTUAL:\n```\n{}\n```\n",
                pat,
                self.stdout()
            )
        }
        self
    }
    fn assert_stdout_not_contains(&self, pat: &str) -> &Self {
        if self.stdout().contains(pat) {
            panic!(
                "`!self.stdout().contains(pat)`:\nPAT:\n```\n{}\n```\n\nACTUAL:\n```\n{}\n```\n",
                pat,
                self.stdout()
            )
        }
        self
    }
}

#[test]
fn test_real() {
    let output = cargo_hack()
        .args(&["hack", "check"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap();

    output
        .assert_success()
        .assert_stderr_not_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("running `cargo check` on member2")
        .assert_stderr_contains("running `cargo check` on real");
}

#[test]
fn test_real_all() {
    let output = cargo_hack()
        .args(&["hack", "check", "--all"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap();

    output
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2")
        .assert_stderr_contains("running `cargo check` on real");
}

#[test]
fn test_real_ignore_private() {
    let output = cargo_hack()
        .args(&["hack", "check", "--ignore-private"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap();

    output
        .assert_success()
        .assert_stderr_not_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("running `cargo check` on member2")
        .assert_stderr_not_contains("running `cargo check` on real")
        .assert_stderr_not_contains("skipped running on member1")
        .assert_stderr_not_contains("skipped running on member2")
        .assert_stderr_contains("skipped running on real");
}

#[test]
fn test_real_ignore_private_all() {
    let output = cargo_hack()
        .args(&["hack", "check", "--all", "--ignore-private"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap();

    output
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("running `cargo check` on member2")
        .assert_stderr_not_contains("running `cargo check` on real")
        .assert_stderr_not_contains("skipped running on member1")
        .assert_stderr_contains("skipped running on member2")
        .assert_stderr_contains("skipped running on real");
}

#[test]
fn test_virtual() {
    let output = cargo_hack()
        .args(&["hack", "check"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap();

    output
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2");
}

#[test]
fn test_virtual_all() {
    let output = cargo_hack()
        .args(&["hack", "check", "--all"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap();

    output
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2");
}

#[test]
fn test_virtual_ignore_private() {
    let output = cargo_hack()
        .args(&["hack", "check", "--ignore-private"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap();

    output
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("running `cargo check` on member2")
        .assert_stderr_not_contains("skipped running on member1")
        .assert_stderr_contains("skipped running on member2");
}

#[test]
fn test_virtual_ignore_private_all() {
    let output = cargo_hack()
        .args(&["hack", "check", "--all", "--ignore-private"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap();

    output
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("running `cargo check` on member2")
        .assert_stderr_not_contains("skipped running on member1")
        .assert_stderr_contains("skipped running on member2");
}

#[test]
fn test_package() {
    let output = cargo_hack()
        .args(&["hack", "check", "--package", "member1"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap();

    output
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("running `cargo check` on member2");
}

#[test]
fn test_package_no_packages() {
    let output = cargo_hack()
        .args(&["hack", "check", "--package", "foo"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap();

    output
        .assert_failure()
        .assert_stderr_contains("package ID specification `foo` matched no packages");
}

#[test]
fn test_exclude() {
    let output = cargo_hack()
        .args(&["hack", "check", "--all", "--exclude", "foo"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap();

    output
        .assert_success()
        .assert_stderr_contains("excluded package(s) foo not found in workspace")
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2");
}

#[test]
fn test_exclude_not_found() {
    let output = cargo_hack()
        .args(&["hack", "check", "--all", "--exclude", "member1"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap();

    output
        .assert_success()
        .assert_stderr_not_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2");
}

#[test]
fn test_exclude_not_all() {
    let output = cargo_hack()
        .args(&["hack", "check", "--exclude", "member1"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap();

    output
        .assert_failure()
        .assert_stderr_contains("--exclude can only be used together with --workspace");
}

#[test]
fn test_ignore_unknown_features() {
    let output = cargo_hack()
        .args(&["hack", "check", "--ignore-unknown-features", "--features=f"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap();

    output
        .assert_success()
        .assert_stderr_contains("skipped applying unknown `f` feature to member1")
        .assert_stderr_contains("skipped applying unknown `f` feature to member2");
}

#[test]
fn test_ignore_non_exist_features() {
    let output = cargo_hack()
        .args(&["hack", "check", "--ignore-non-exist-features", "--features=f"])
        .current_dir(test_dir("tests/fixtures/virtual"))
        .output()
        .unwrap();

    output
        .assert_success()
        .assert_stderr_contains("'--ignore-non-exist-features' flag is deprecated, use '--ignore-unknown-features' flag instead")
        .assert_stderr_contains("skipped applying unknown `f` feature to member1")
        .assert_stderr_contains("skipped applying unknown `f` feature to member2");
}

#[test]
fn test_each_feature() {
    let output = cargo_hack()
        .args(&["hack", "check", "--each-feature"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap();

    output
        .assert_success()
        .assert_stderr_contains("running `cargo check` on real")
        .assert_stderr_contains("running `cargo check --no-default-features` on real")
        .assert_stderr_contains("running `cargo check --features=a --no-default-features` on real")
        .assert_stderr_contains("running `cargo check --features=b --no-default-features` on real")
        .assert_stderr_contains("running `cargo check --features=c --no-default-features` on real");
}

#[test]
fn test_args2() {
    let output = cargo_hack()
        .args(&["hack", "test", "--", "--ignored"])
        .current_dir(test_dir("tests/fixtures/real"))
        .output()
        .unwrap();

    output
        .assert_success()
        .assert_stderr_contains("cargo test -- --ignored")
        .assert_stdout_contains("running 1 test")
        .assert_stdout_contains("test tests::test_ignored");
}

#[test]
fn windows_package_collision() {
    let output = cargo_hack()
        .args(&["hack", "check"])
        .current_dir(test_dir("tests/fixtures/windows_package_collision"))
        .output()
        .unwrap();

    if cfg!(windows) {
        output
            .assert_failure()
            .assert_stderr_contains("package collision in the lockfile: packages member2");
    } else {
        output
            .assert_success()
            .assert_stderr_contains("running `cargo check` on member1")
            .assert_stderr_contains("running `cargo check` on member2");
    }
}

#[test]
fn windows_not_find_manifest() {
    let output = cargo_hack()
        .args(&["hack", "check"])
        .current_dir(test_dir("tests/fixtures/windows_not_find_manifest"))
        .output()
        .unwrap();

    if cfg!(windows) {
        output.assert_failure().assert_stderr_contains("Could not find `Cargo.toml` in `");
    } else {
        output
            .assert_success()
            .assert_stderr_contains("running `cargo check` on member1")
            .assert_stderr_contains("running `cargo check` on member2");
    }
}
