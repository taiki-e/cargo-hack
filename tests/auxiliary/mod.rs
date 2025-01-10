// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    env,
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    str,
    sync::LazyLock,
};

use anyhow::Context as _;
pub(crate) use build_context::TARGET;
use easy_ext::ext;
use fs_err as fs;

pub(crate) fn fixtures_path() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
}

pub(crate) fn cargo_bin_exe() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_cargo-hack"));
    cmd.env("CARGO_HACK_DENY_WARNINGS", "1");
    cmd.env_remove("RUSTFLAGS");
    cmd.env_remove("CARGO_TERM_COLOR");
    cmd.env_remove("GITHUB_ACTIONS");
    cmd
}

pub(crate) fn has_rustup() -> bool {
    Command::new("rustup").arg("--version").output().is_ok()
}

static TEST_VERSION: LazyLock<Option<u32>> = LazyLock::new(|| {
    let toolchain = env::var_os("CARGO_HACK_TEST_TOOLCHAIN")?.to_string_lossy().parse().unwrap();
    // Install toolchain first to avoid toolchain installation conflicts.
    let _ = Command::new("rustup")
        .args(["toolchain", "add", &format!("1.{toolchain}"), "--no-self-update"])
        .output();
    Some(toolchain)
});

pub(crate) static HAS_STABLE_TOOLCHAIN: LazyLock<bool> = LazyLock::new(|| {
    let Ok(output) = Command::new("rustup").args(["toolchain", "list"]).output() else {
        return false;
    };
    String::from_utf8(output.stdout).unwrap_or_default().contains("stable")
});

pub(crate) fn cargo_hack<O: AsRef<OsStr>>(args: impl AsRef<[O]>) -> Command {
    let args = args.as_ref();
    let mut cmd = cargo_bin_exe();
    cmd.arg("hack");
    if let Some(toolchain) = *TEST_VERSION {
        if !args.iter().any(|a| {
            let s = a.as_ref().to_str().unwrap();
            s.starts_with("--version-range") || s.starts_with("--rust-version")
        }) {
            cmd.arg(format!("--version-range=1.{toolchain}..=1.{toolchain}"));
        }
    }
    cmd.args(args);
    cmd
}

#[ext(CommandExt)]
impl Command {
    #[track_caller]
    pub(crate) fn assert_output(&mut self, test_model: &str, require: Option<u32>) -> AssertOutput {
        match (*TEST_VERSION, require) {
            (Some(toolchain), Some(require)) if require > toolchain => {
                return AssertOutput(None);
            }
            _ => {}
        }
        let (_test_project, cur_dir) = test_project(test_model);
        let output =
            self.current_dir(cur_dir).output().context("could not execute process").unwrap();
        AssertOutput(Some(AssertOutputInner {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr)
                .lines()
                .filter(|l| {
                    // https://github.com/taiki-e/cargo-hack/issues/239
                    !(l.starts_with("warning:")
                        && l.contains(": no edition set: defaulting to the 2015 edition"))
                })
                .collect(),
            status: output.status,
        }))
    }

    #[track_caller]
    pub(crate) fn assert_success(&mut self, test_model: &str) -> AssertOutput {
        self.assert_success2(test_model, None)
    }

    #[track_caller]
    pub(crate) fn assert_success2(
        &mut self,
        test_model: &str,
        require: Option<u32>,
    ) -> AssertOutput {
        let output = self.assert_output(test_model, require);
        if let Some(output) = &output.0 {
            if !output.status.success() {
                panic!(
                    "assertion failed: `self.status.success()`:\n\nSTDOUT:\n{0}\n{1}\n{0}\n\nSTDERR:\n{0}\n{2}\n{0}\n",
                    "-".repeat(60),
                    output.stdout,
                    output.stderr,
                );
            }
        }
        output
    }

    #[track_caller]
    pub(crate) fn assert_failure(&mut self, test_model: &str) -> AssertOutput {
        self.assert_failure2(test_model, None)
    }

    #[track_caller]
    pub(crate) fn assert_failure2(
        &mut self,
        test_model: &str,
        require: Option<u32>,
    ) -> AssertOutput {
        let output = self.assert_output(test_model, require);
        if let Some(output) = &output.0 {
            if output.status.success() {
                panic!(
                    "assertion failed: `!self.status.success()`:\n\nSTDOUT:\n{0}\n{1}\n{0}\n\nSTDERR:\n{0}\n{2}\n{0}\n",
                    "-".repeat(60),
                    output.stdout,
                    output.stderr,
                );
            }
        }
        output
    }
}

pub(crate) struct AssertOutput(Option<AssertOutputInner>);

struct AssertOutputInner {
    stdout: String,
    stderr: String,
    status: ExitStatus,
}

fn replace_command(lines: &str) -> String {
    if lines.contains("rustup run") {
        lines.to_owned()
    } else if let Some(minor) = *TEST_VERSION {
        lines.replace("cargo ", &format!("rustup run 1.{minor} cargo "))
    } else {
        lines.to_owned()
    }
}
fn line_separated(lines: &str) -> impl Iterator<Item = &'_ str> {
    lines.lines().map(str::trim).filter(|line| !line.is_empty())
}

impl AssertOutput {
    /// Receives a line(`\n`)-separated list of patterns and asserts whether stderr contains each pattern.
    #[track_caller]
    pub(crate) fn stderr_contains(&self, pats: impl AsRef<str>) -> &Self {
        if let Some(output) = &self.0 {
            for pat in line_separated(&replace_command(pats.as_ref())) {
                if !output.stderr.contains(pat) {
                    panic!(
                        "assertion failed: `self.stderr.contains(..)`:\n\nEXPECTED:\n{0}\n{pat}\n{0}\n\nACTUAL:\n{0}\n{1}\n{0}\n",
                        "-".repeat(60),
                        output.stderr
                    );
                }
            }
        }
        self
    }

    /// Receives a line(`\n`)-separated list of patterns and asserts whether stdout contains each pattern.
    #[track_caller]
    pub(crate) fn stderr_not_contains(&self, pats: impl AsRef<str>) -> &Self {
        if let Some(output) = &self.0 {
            for pat in line_separated(&replace_command(pats.as_ref())) {
                if output.stderr.contains(pat) {
                    panic!(
                        "assertion failed: `!self.stderr.contains(..)`:\n\nEXPECTED:\n{0}\n{pat}\n{0}\n\nACTUAL:\n{0}\n{1}\n{0}\n",
                        "-".repeat(60),
                        output.stderr
                    );
                }
            }
        }
        self
    }

    /// Receives a line(`\n`)-separated list of patterns and asserts whether stdout contains each pattern.
    #[track_caller]
    pub(crate) fn stdout_contains(&self, pats: impl AsRef<str>) -> &Self {
        if let Some(output) = &self.0 {
            for pat in line_separated(&replace_command(pats.as_ref())) {
                if !output.stdout.contains(pat) {
                    panic!(
                        "assertion failed: `self.stdout.contains(..)`:\n\nEXPECTED:\n{0}\n{pat}\n{0}\n\nACTUAL:\n{0}\n{1}\n{0}\n",
                        "-".repeat(60),
                        output.stdout
                    );
                }
            }
        }
        self
    }

    /// Receives a line(`\n`)-separated list of patterns and asserts whether stdout contains each pattern.
    #[track_caller]
    pub(crate) fn stdout_not_contains(&self, pats: impl AsRef<str>) -> &Self {
        if let Some(output) = &self.0 {
            for pat in line_separated(&replace_command(pats.as_ref())) {
                if output.stdout.contains(pat) {
                    panic!(
                        "assertion failed: `!self.stdout.contains(..)`:\n\nEXPECTED:\n{0}\n{pat}\n{0}\n\nACTUAL:\n{0}\n{1}\n{0}\n",
                        "-".repeat(60),
                        output.stdout
                    );
                }
            }
        }
        self
    }
}

#[track_caller]
fn test_project(model: &str) -> (tempfile::TempDir, PathBuf) {
    let tmpdir = tempfile::tempdir().unwrap();
    let tmpdir_path = tmpdir.path();

    let model_path;
    let workspace_root;
    if model.contains('/') {
        let mut model = model.splitn(2, '/');
        model_path = fixtures_path().join(model.next().unwrap());
        workspace_root = tmpdir_path.join(model.next().unwrap());
        assert!(model.next().is_none());
    } else {
        model_path = fixtures_path().join(model);
        workspace_root = tmpdir_path.to_path_buf();
    }

    for (file_name, from) in git_ls_files(&model_path, &[]) {
        let to = &tmpdir_path.join(file_name);
        if !to.parent().unwrap().is_dir() {
            fs::create_dir_all(to.parent().unwrap()).unwrap();
        }
        fs::copy(from, to).unwrap();
    }

    (tmpdir, workspace_root)
}

#[track_caller]
fn git_ls_files(dir: &Path, filters: &[&str]) -> Vec<(String, PathBuf)> {
    let mut cmd = Command::new("git");
    cmd.arg("ls-files").args(filters).current_dir(dir);
    let output =
        cmd.output().unwrap_or_else(|e| panic!("could not execute process `{cmd:?}`: {e}"));
    assert!(
            output.status.success(),
            "process didn't exit successfully: `{cmd:?}`:\n\nSTDOUT:\n{0}\n{1}\n{0}\n\nSTDERR:\n{0}\n{2}\n{0}\n",
            "-".repeat(60),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    str::from_utf8(&output.stdout)
        .unwrap()
        .lines()
        .map(str::trim)
        .filter_map(|f| {
            if f.is_empty() {
                return None;
            }
            let p = dir.join(f);
            if !p.exists() {
                return None;
            }
            Some((f.to_owned(), p))
        })
        .collect()
}
