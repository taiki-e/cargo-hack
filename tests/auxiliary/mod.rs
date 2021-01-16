use easy_ext::ext;
use std::{
    env,
    ffi::OsStr,
    path::{self, Path},
    process::{Command, ExitStatus},
};

pub use path::MAIN_SEPARATOR as SEPARATOR;

pub fn cargo_bin_exe() -> Command {
    // TODO: update to use `env!("CARGO_BIN_EXE_cargo-hack")`
    // https://github.com/rust-lang/cargo/pull/7697, require Rust 1.43
    let mut exe = env::current_exe().unwrap();
    exe.pop();
    if exe.ends_with("deps") {
        exe.pop();
    }
    exe.push("cargo-hack");
    Command::new(exe)
}

pub fn cargo_hack<O: AsRef<OsStr>>(args: impl AsRef<[O]>) -> Command {
    let mut cmd = cargo_bin_exe();
    cmd.arg("hack");
    cmd.args(args.as_ref());
    cmd
}

#[ext(CommandExt)]
impl Command {
    pub fn test_dir(&mut self, path: impl AsRef<Path>) -> &mut Self {
        self.current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
    }

    #[rustversion::attr(since(1.46), track_caller)]
    pub fn assert_output(&mut self) -> AssertOutput {
        let output = self.output().unwrap_or_else(|e| panic!("could not execute process: {}", e));
        AssertOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            status: output.status,
        }
    }

    #[rustversion::attr(since(1.46), track_caller)]
    pub fn assert_success(&mut self) -> AssertOutput {
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

    #[rustversion::attr(since(1.46), track_caller)]
    pub fn assert_failure(&mut self) -> AssertOutput {
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

pub struct AssertOutput {
    stdout: String,
    stderr: String,
    status: ExitStatus,
}

fn line_separated(lines: &str, f: impl FnMut(&str)) {
    lines.split('\n').map(str::trim).filter(|line| !line.is_empty()).for_each(f);
}

impl AssertOutput {
    /// Receives a line(`\n`)-separated list of patterns and asserts whether stderr contains each pattern.
    #[rustversion::attr(since(1.46), track_caller)]
    pub fn stderr_contains(&self, pats: impl AsRef<str>) -> &Self {
        line_separated(pats.as_ref(), |pat| {
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
    #[rustversion::attr(since(1.46), track_caller)]
    pub fn stderr_not_contains(&self, pats: impl AsRef<str>) -> &Self {
        line_separated(pats.as_ref(), |pat| {
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
    #[rustversion::attr(since(1.46), track_caller)]
    pub fn stdout_contains(&self, pats: impl AsRef<str>) -> &Self {
        line_separated(pats.as_ref(), |pat| {
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
    #[rustversion::attr(since(1.46), track_caller)]
    pub fn stdout_not_contains(&self, pats: impl AsRef<str>) -> &Self {
        line_separated(pats.as_ref(), |pat| {
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

pub fn target_triple() -> &'static str {
    if cfg!(not(target_arch = "x86_64")) {
        panic!("non x86_64 arch")
    }
    if cfg!(target_os = "linux") {
        if cfg!(target_env = "gnu") {
            "x86_64-unknown-linux-gnu"
        } else if cfg!(target_env = "musl") {
            "x86_64-unknown-linux-musl"
        } else {
            panic!("non gnu/musl linux")
        }
    } else if cfg!(target_os = "macos") {
        "x86_64-apple-darwin"
    } else if cfg!(target_os = "windows") {
        if cfg!(target_env = "gnu") {
            "x86_64-pc-windows-gnu"
        } else if cfg!(target_env = "msvc") {
            "x86_64-pc-windows-msvc"
        } else {
            unreachable!()
        }
    } else {
        panic!("non linux/macos/windows os")
    }
}
