#![allow(dead_code)]

//! Shared CLI integration-test harness for `chatter`.

pub mod command_surface;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::Value;
use talkbank_parser_tests::test_error::TestError;
use tempfile::{TempDir, tempdir};

/// Cache root shared by every CLI subprocess that does not stand up its own
/// [`CliHarness`].
///
/// Created once per test binary and kept alive for its whole run.
static SHARED_CACHE_DIR: std::sync::OnceLock<TempDir> = std::sync::OnceLock::new();

/// Build a `chatter` command whose cache CANNOT be the developer's real one.
///
/// # Why this exists rather than `cargo_bin_cmd!` at each call site
///
/// A spawned `chatter validate` writes to, and now prunes, whatever cache
/// `TALKBANK_CHAT_CACHE_DIR` (or the platform default) resolves to. Tests that
/// spawned the binary directly therefore ran against the machine's real cache:
/// their verdicts depended on state no test wrote, and they mutated a
/// user-owned artifact as a side effect. That was invisible until reachability
/// pruning landed, at which point one `cargo test` run deleted a real corpus
/// cache. Isolation is not something each test should have to remember, so the
/// only builder tests reach for provides it.
pub fn chatter_cmd() -> assert_cmd::Command {
    let cache_dir = SHARED_CACHE_DIR
        .get_or_init(|| tempdir().expect("create shared test cache dir"))
        .path();
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("chatter");
    cmd.env("TALKBANK_CHAT_CACHE_DIR", cache_dir);
    cmd
}

/// Isolated integration-test harness for running the `chatter` binary.
#[allow(dead_code)]
pub struct CliHarness {
    _scratch: TempDir,
    home_dir: PathBuf,
    xdg_cache_home: PathBuf,
}

#[allow(dead_code)]
impl CliHarness {
    /// Create a new harness with isolated HOME and XDG cache roots.
    pub fn new() -> Result<Self, TestError> {
        let scratch = tempdir()?;
        let home_dir = scratch.path().join("home");
        let xdg_cache_home = home_dir.join(".cache");
        fs::create_dir_all(&xdg_cache_home)?;

        Ok(Self {
            _scratch: scratch,
            home_dir,
            xdg_cache_home,
        })
    }

    /// Build a `chatter` command configured to avoid user-machine cache state.
    ///
    /// `HOME` / `XDG_CACHE_HOME` isolate the cache on macOS and Linux,
    /// but Windows resolves the platform cache root through the Known
    /// Folder API and ignores both, so all tests shared one real cache
    /// there (racy entry counts, cross-platform CI 2026-06-12).
    /// `TALKBANK_CHAT_CACHE_DIR` is the explicit override that makes
    /// the isolation deterministic on every platform.
    pub fn chatter_cmd(&self) -> assert_cmd::Command {
        let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("chatter");
        cmd.env("HOME", self.home_dir());
        cmd.env("XDG_CACHE_HOME", self.xdg_cache_home());
        cmd.env(
            "TALKBANK_CHAT_CACHE_DIR",
            self.xdg_cache_home().join("talkbank-chat"),
        );
        cmd
    }

    /// Run `chatter` and capture the subprocess output.
    pub fn run_output(&self, args: &[&str]) -> Result<Output, TestError> {
        Ok(self.chatter_cmd().args(args).output()?)
    }

    /// Run `chatter validate` for a file or directory path.
    pub fn run_validate(&self, path: &Path, extra_args: &[&str]) -> Result<Output, TestError> {
        let mut cmd = self.chatter_cmd();
        cmd.arg("validate");
        cmd.args(extra_args);
        cmd.arg(path);
        Ok(cmd.output()?)
    }

    /// HOME directory injected into the CLI process.
    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }

    /// XDG cache root injected into the CLI process.
    pub fn xdg_cache_home(&self) -> &Path {
        &self.xdg_cache_home
    }
}

/// Resolve a workspace-relative path (such as a `corpus/reference/...`
/// fixture) to an absolute path.
///
/// Integration tests run with the crate directory as the working directory,
/// so workspace paths resolve relative to the workspace root, two levels up
/// from `CARGO_MANIFEST_DIR`. Shared here so individual test files do not each
/// re-roll the same `CARGO_MANIFEST_DIR/../..` boilerplate.
pub fn reference_fixture(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

/// Write one test fixture relative to a temp root, creating parent dirs first.
pub fn write_fixture(path: &Path, relative: &str, content: &str) -> Result<PathBuf, TestError> {
    let file_path = path.join(relative);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&file_path, content)?;
    Ok(file_path)
}

/// Decode one subprocess stdout payload as UTF-8 lossily.
pub fn stdout_string(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Decode one subprocess stderr payload as UTF-8 lossily.
pub fn stderr_string(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Decode stdout and stderr together, for assertions that must match a
/// diagnostic regardless of which stream it lands on (the target stream varies
/// with `--format` / `--quiet`).
pub fn combined_output(output: &Output) -> String {
    format!("{}{}", stdout_string(output), stderr_string(output))
}

/// Assert a CLI subprocess succeeded and print captured output on failure.
pub fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} failed\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        stdout_string(output),
        stderr_string(output)
    );
}

/// Assert a CLI subprocess failed and print captured output if it unexpectedly passed.
pub fn assert_failure(output: &Output, context: &str) {
    assert!(
        !output.status.success(),
        "{context} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        stdout_string(output),
        stderr_string(output)
    );
}

/// Parse one subprocess stdout payload as JSON.
pub fn parse_json(output: &Output) -> Result<Value, TestError> {
    serde_json::from_slice(&output.stdout)
        .map_err(|error| TestError::Failure(format!("expected JSON output: {error}")))
}

/// What `chatter validate` must say about a fixture.
///
/// This replaces a bare `valid: bool` that three integration modules each
/// carried a copy of. `false` asserted only that SOMETHING was reported, so a
/// test written for one rule passed when a different rule fired instead, and
/// nothing in the source said which rule the test was actually about.
///
/// There is deliberately no "invalid, code unchecked" variant. One existed for
/// a few hours as a migration staging post while the 11 inherited `false` call
/// sites were converted; each one's real code was then determined by running
/// the fixture (E220 for digit-bearing words, E762 and E763 for the two
/// prefix-marker rules), and the variant was deleted. A weak assertion nothing
/// can express is better than one that is merely discouraged.
pub enum Verdict {
    Valid,
    /// Invalid, and this specific code must appear. Naming the code by VARIANT
    /// rather than by a string means retiring or renaming it breaks the test at
    /// compile time instead of silently matching nothing.
    Rejected(talkbank_model::ErrorCode),
}

/// Write `content` to a temp file and assert `chatter validate`'s verdict.
///
/// The count is on stdout and the diagnostic on stderr, so a specific-code
/// expectation asserts on both streams: `Invalid: 1` pins that the file was
/// rejected at invalidity severity, and `error[CODE]` pins which rule did it.
/// Neither implies the other.
pub fn assert_validation(name: &str, content: &str, expected: Verdict) -> Result<(), TestError> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join(name);
    std::fs::write(&path, content)?;

    let assertion = chatter_cmd().arg("validate").arg(&path).assert();
    match expected {
        Verdict::Valid => {
            assertion
                .success()
                .stdout(predicates::str::contains("Invalid: 0"));
        }
        Verdict::Rejected(code) => {
            assertion
                .failure()
                .stdout(predicates::str::contains("Invalid: 1"))
                .stderr(predicates::str::contains(format!(
                    "error[{}]",
                    code.as_str()
                )));
        }
    }
    Ok(())
}
