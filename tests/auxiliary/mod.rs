pub use std::path::MAIN_SEPARATOR as SEPARATOR;
use std::{
    env,
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};

use anyhow::Result;
use easy_ext::ext;
use fs_err as fs;
use once_cell::sync::Lazy;
use tempfile::{Builder, TempDir};
use walkdir::WalkDir;

static FIXTURES_PATH: Lazy<PathBuf> =
    Lazy::new(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"));

pub fn cargo_bin_exe() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_cargo-hack"));
    cmd.env_remove("RUSTFLAGS");
    cmd
}

fn test_toolchain() -> String {
    if let Some(toolchain) = test_version() { format!(" +1.{}", toolchain) } else { String::new() }
}

fn test_version() -> Option<u32> {
    static TEST_VERSION: Lazy<Option<u32>> = Lazy::new(|| {
        let toolchain =
            env::var_os("CARGO_HACK_TEST_TOOLCHAIN")?.to_string_lossy().parse().unwrap();
        // Install toolchain first to avoid toolchain installation conflicts.
        let _ = Command::new("rustup")
            .args(&["toolchain", "install", &format!("1.{}", toolchain), "--no-self-update"])
            .output();
        Some(toolchain)
    });
    *TEST_VERSION
}

pub fn cargo_hack<O: AsRef<OsStr>>(args: impl AsRef<[O]>) -> Command {
    let args = args.as_ref();
    let mut cmd = cargo_bin_exe();
    cmd.arg("hack");
    if let Some(toolchain) = test_version() {
        if !args.iter().any(|a| a.as_ref().to_str().unwrap().starts_with("--version-range")) {
            cmd.arg(format!("--version-range=1.{0}..1.{0}", toolchain));
        }
    }
    cmd.args(args);
    cmd
}

#[ext(CommandExt)]
impl Command {
    #[track_caller]
    pub fn assert_output(&mut self, test_model: &str, require: Option<u32>) -> AssertOutput {
        match (test_version(), require) {
            (Some(toolchain), Some(require)) if require > toolchain => {
                return AssertOutput(None);
            }
            _ => {}
        }
        let (_test_project, cur_dir) = test_project(test_model).unwrap();
        let output = self
            .current_dir(cur_dir)
            .output()
            .unwrap_or_else(|e| panic!("could not execute process: {}", e));
        AssertOutput(Some(AssertOutputInner {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            status: output.status,
        }))
    }

    #[track_caller]
    pub fn assert_success(&mut self, test_model: &str) -> AssertOutput {
        self.assert_success2(test_model, None)
    }

    #[track_caller]
    pub fn assert_success2(&mut self, test_model: &str, require: Option<u32>) -> AssertOutput {
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
    pub fn assert_failure(&mut self, test_model: &str) -> AssertOutput {
        self.assert_failure2(test_model, None)
    }

    #[track_caller]
    pub fn assert_failure2(&mut self, test_model: &str, require: Option<u32>) -> AssertOutput {
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

pub struct AssertOutput(Option<AssertOutputInner>);

struct AssertOutputInner {
    stdout: String,
    stderr: String,
    status: ExitStatus,
}

#[track_caller]
fn line_separated(lines: &str, f: impl FnMut(&str)) {
    let lines = if lines.contains("`cargo +") {
        lines.to_string()
    } else {
        lines.replace("`cargo", &format!("`cargo{}", test_toolchain()))
    };
    lines.split('\n').map(str::trim).filter(|line| !line.is_empty()).for_each(f);
}

impl AssertOutput {
    /// Receives a line(`\n`)-separated list of patterns and asserts whether stderr contains each pattern.
    #[track_caller]
    pub fn stderr_contains(&self, pats: impl AsRef<str>) -> &Self {
        if let Some(output) = &self.0 {
            line_separated(pats.as_ref(), |pat| {
                if !output.stderr.contains(pat) {
                    panic!(
                        "assertion failed: `self.stderr.contains(..)`:\n\nEXPECTED:\n{0}\n{1}\n{0}\n\nACTUAL:\n{0}\n{2}\n{0}\n",
                        "-".repeat(60),
                        pat,
                        output.stderr
                    );
                }
            });
        }
        self
    }

    /// Receives a line(`\n`)-separated list of patterns and asserts whether stdout contains each pattern.
    #[track_caller]
    pub fn stderr_not_contains(&self, pats: impl AsRef<str>) -> &Self {
        if let Some(output) = &self.0 {
            line_separated(pats.as_ref(), |pat| {
                if output.stderr.contains(pat) {
                    panic!(
                        "assertion failed: `!self.stderr.contains(..)`:\n\nEXPECTED:\n{0}\n{1}\n{0}\n\nACTUAL:\n{0}\n{2}\n{0}\n",
                        "-".repeat(60),
                        pat,
                        output.stderr
                    );
                }
            });
        }
        self
    }

    /// Receives a line(`\n`)-separated list of patterns and asserts whether stdout contains each pattern.
    #[track_caller]
    pub fn stdout_contains(&self, pats: impl AsRef<str>) -> &Self {
        if let Some(output) = &self.0 {
            line_separated(pats.as_ref(), |pat| {
                if !output.stdout.contains(pat) {
                    panic!(
                        "assertion failed: `self.stdout.contains(..)`:\n\nEXPECTED:\n{0}\n{1}\n{0}\n\nACTUAL:\n{0}\n{2}\n{0}\n",
                        "-".repeat(60),
                        pat,
                        output.stdout
                    );
                }
            });
        }
        self
    }

    /// Receives a line(`\n`)-separated list of patterns and asserts whether stdout contains each pattern.
    #[track_caller]
    pub fn stdout_not_contains(&self, pats: impl AsRef<str>) -> &Self {
        if let Some(output) = &self.0 {
            line_separated(pats.as_ref(), |pat| {
                if output.stdout.contains(pat) {
                    panic!(
                        "assertion failed: `!self.stdout.contains(..)`:\n\nEXPECTED:\n{0}\n{1}\n{0}\n\nACTUAL:\n{0}\n{2}\n{0}\n",
                        "-".repeat(60),
                        pat,
                        output.stdout
                    );
                }
            });
        }
        self
    }
}

pub fn target_triple() -> String {
    let triple = env::consts::ARCH.to_string();
    if cfg!(target_os = "macos") {
        triple + "-apple-darwin"
    } else if cfg!(target_os = "windows") {
        if cfg!(target_env = "gnu") {
            triple + "-pc-windows-gnu"
        } else if cfg!(target_env = "msvc") {
            triple + "-pc-windows-msvc"
        } else {
            unreachable!()
        }
    } else if cfg!(target_env = "gnu") {
        triple + "-unknown-" + env::consts::OS + "-gnu"
    } else if cfg!(target_env = "musl") {
        triple + "-unknown-" + env::consts::OS + "-musl"
    } else {
        unreachable!()
    }
}

fn test_project(model: &str) -> Result<(TempDir, PathBuf)> {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    let tmpdir = Builder::new()
        .prefix(&format!("test_project{}", COUNTER.fetch_add(1, Relaxed)))
        .tempdir()?;
    let tmpdir_path = tmpdir.path();

    let model_path;
    let workspace_root;
    if model.contains('/') {
        let mut model = model.splitn(2, '/');
        model_path = FIXTURES_PATH.join(model.next().unwrap());
        workspace_root = tmpdir_path.join(model.next().unwrap());
        assert!(model.next().is_none());
    } else {
        model_path = FIXTURES_PATH.join(model);
        workspace_root = tmpdir_path.to_path_buf();
    }

    for entry in WalkDir::new(&model_path).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        let tmppath = &tmpdir_path.join(path.strip_prefix(&model_path)?);
        if !tmppath.exists() {
            if path.is_dir() {
                fs::create_dir(tmppath)?;
            } else {
                fs::copy(path, tmppath)?;
            }
        }
    }

    Ok((tmpdir, workspace_root))
}
