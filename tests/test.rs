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

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[easy_ext::ext(OutputExt)]
impl Output {
    fn stdout(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.stdout)
    }
    fn stderr(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.stderr)
    }
    fn assert_success(&self) {
        if !self.status.success() {
            panic!(
                "`self.status.success()`:\nSTDOUT:\n```\n{}\n```\n\nSTDERR:\n```\n{}\n```\n",
                self.stdout(),
                self.stderr(),
            )
        }
    }
    fn assert_failure(&self) {
        if self.status.success() {
            panic!(
                "`!self.status.success()`:\nSTDOUT:\n```\n{}\n```\n\nSTDERR:\n```\n{}\n```\n",
                self.stdout(),
                self.stderr(),
            )
        }
    }
    fn assert_stderr_contains(&self, pat: impl AsRef<str>) {
        if !self.stderr().contains(pat.as_ref()) {
            panic!(
                "`self.stderr().contains(pat)`:\nPAT:\n```\n{}\n```\n\nACTUAL:\n```\n{}\n```\n",
                pat.as_ref(),
                self.stderr()
            )
        }
    }
    fn assert_stderr_not_contains(&self, pat: impl AsRef<str>) {
        if self.stderr().contains(pat.as_ref()) {
            panic!(
                "`!self.stderr().contains(pat)`:\nPAT:\n```\n{}\n```\n\nACTUAL:\n```\n{}\n```\n",
                pat.as_ref(),
                self.stderr()
            )
        }
    }
    // fn assert_stdout_contains(&self, pat: impl AsRef<str>)
    // fn assert_stderr_contains_exact(&self, pat: impl AsRef<str>)
    // fn assert_stdout_contains_exact(&self, pat: impl AsRef<str>)
}

#[test]
fn test_real() {
    let current_dir = manifest_dir().join("tests/fixtures/real");
    let output = cargo_hack().args(&["hack", "check"]).current_dir(current_dir).output().unwrap();

    output.assert_success();
    output.assert_stderr_not_contains("running `cargo check` on member1");
    output.assert_stderr_not_contains("running `cargo check` on member2");
    output.assert_stderr_contains("running `cargo check` on real");
}

#[test]
fn test_real_all() {
    let current_dir = manifest_dir().join("tests/fixtures/real");
    let output =
        cargo_hack().args(&["hack", "check", "--all"]).current_dir(current_dir).output().unwrap();

    output.assert_success();
    output.assert_stderr_contains("`--all` flag for `cargo hack` is experimental");
    output.assert_stderr_contains("running `cargo check` on member1");
    output.assert_stderr_contains("running `cargo check` on member2");
    output.assert_stderr_contains("running `cargo check` on real");
}

#[test]
fn test_real_ignore_private() {
    let current_dir = manifest_dir().join("tests/fixtures/real");
    let output = cargo_hack()
        .args(&["hack", "check", "--ignore-private"])
        .current_dir(current_dir)
        .output()
        .unwrap();

    output.assert_success();
    output.assert_stderr_not_contains("running `cargo check` on member1");
    output.assert_stderr_not_contains("running `cargo check` on member2");
    output.assert_stderr_not_contains("running `cargo check` on real");
    output.assert_stderr_not_contains("skipped running on member1");
    output.assert_stderr_not_contains("skipped running on member2");
    output.assert_stderr_contains("skipped running on real");
}

#[test]
fn test_real_ignore_private_all() {
    let current_dir = manifest_dir().join("tests/fixtures/real");
    let output = cargo_hack()
        .args(&["hack", "check", "--all", "--ignore-private"])
        .current_dir(current_dir)
        .output()
        .unwrap();

    output.assert_success();
    output.assert_stderr_contains("`--all` flag for `cargo hack` is experimental");
    output.assert_stderr_contains("running `cargo check` on member1");
    output.assert_stderr_not_contains("running `cargo check` on member2");
    output.assert_stderr_not_contains("running `cargo check` on real");
    output.assert_stderr_not_contains("skipped running on member1");
    output.assert_stderr_contains("skipped running on member2");
    output.assert_stderr_contains("skipped running on real");
}

#[test]
fn test_virtual() {
    let current_dir = manifest_dir().join("tests/fixtures/virtual");
    let output = cargo_hack().args(&["hack", "check"]).current_dir(current_dir).output().unwrap();

    output.assert_success();
    output.assert_stderr_contains("running `cargo check` on member1");
    output.assert_stderr_contains("running `cargo check` on member2");
}

#[test]
fn test_virtual_all() {
    let current_dir = manifest_dir().join("tests/fixtures/virtual");
    let output =
        cargo_hack().args(&["hack", "check", "--all"]).current_dir(current_dir).output().unwrap();

    output.assert_success();
    output.assert_stderr_contains("`--all` flag for `cargo hack` is experimental");
    output.assert_stderr_contains("running `cargo check` on member1");
    output.assert_stderr_contains("running `cargo check` on member2");
}

#[test]
fn test_virtual_ignore_private() {
    let current_dir = manifest_dir().join("tests/fixtures/virtual");
    let output = cargo_hack()
        .args(&["hack", "check", "--ignore-private"])
        .current_dir(current_dir)
        .output()
        .unwrap();

    output.assert_success();
    output.assert_stderr_contains("running `cargo check` on member1");
    output.assert_stderr_not_contains("running `cargo check` on member2");
    output.assert_stderr_not_contains("skipped running on member1");
    output.assert_stderr_contains("skipped running on member2");
}

#[test]
fn test_virtual_ignore_private_all() {
    let current_dir = manifest_dir().join("tests/fixtures/virtual");
    let output = cargo_hack()
        .args(&["hack", "check", "--all", "--ignore-private"])
        .current_dir(current_dir)
        .output()
        .unwrap();

    output.assert_success();
    output.assert_stderr_contains("`--all` flag for `cargo hack` is experimental");
    output.assert_stderr_contains("running `cargo check` on member1");
    output.assert_stderr_not_contains("running `cargo check` on member2");
    output.assert_stderr_not_contains("skipped running on member1");
    output.assert_stderr_contains("skipped running on member2");
}

#[test]
fn test_package() {
    let current_dir = manifest_dir().join("tests/fixtures/real");
    let output = cargo_hack()
        .args(&["hack", "check", "--package", "foo"])
        .current_dir(current_dir)
        .output()
        .unwrap();

    output.assert_success();
    output.assert_stderr_contains("`--package` flag for `cargo hack` is currently ignored");
}

#[test]
fn test_exclude() {
    let current_dir = manifest_dir().join("tests/fixtures/real");
    let output = cargo_hack()
        .args(&["hack", "check", "--exclude", "foo"])
        .current_dir(current_dir)
        .output()
        .unwrap();

    output.assert_success();
    output.assert_stderr_contains("`--exclude` flag for `cargo hack` is currently ignored");
}

#[test]
fn test_ignore_non_exist_features() {
    let current_dir = manifest_dir().join("tests/fixtures/virtual");
    let output = cargo_hack()
        .args(&["hack", "check", "--ignore-non-exist-features", "--features=f"])
        .current_dir(current_dir)
        .output()
        .unwrap();

    output.assert_success();
    output.assert_stderr_contains("skipped applying non-exist `f` feature to member1");
    output.assert_stderr_contains("skipped applying non-exist `f` feature to member2");
}

#[test]
fn test_each_feature() {
    let current_dir = manifest_dir().join("tests/fixtures/real");
    let output = cargo_hack()
        .args(&["hack", "check", "--each-feature"])
        .current_dir(current_dir)
        .output()
        .unwrap();

    output.assert_success();
    output.assert_stderr_contains("running `cargo check` on real");
    output.assert_stderr_contains("running `cargo check --no-default-features` on real");
    output
        .assert_stderr_contains("running `cargo check --features=a --no-default-features` on real");
    output
        .assert_stderr_contains("running `cargo check --features=b --no-default-features` on real");
    output
        .assert_stderr_contains("running `cargo check --features=c --no-default-features` on real");
}
