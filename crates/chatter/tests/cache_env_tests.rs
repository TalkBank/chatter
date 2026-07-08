// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Integration tests for the `TALKBANK_CHAT_CACHE_DIR` cache-root override.
//!
//! Why this exists: the default cache root comes from `dirs::cache_dir()`,
//! which honors `HOME` / `XDG_CACHE_HOME` on macOS and Linux but resolves
//! through the Known Folder API on Windows, where environment variables are
//! ignored. Tests that relied on `HOME`-based isolation therefore shared
//! one real cache on Windows runners, producing racy entry counts
//! (cross-platform CI, 2026-06-12). The explicit override gives users a
//! supported relocation knob and gives tests deterministic isolation on
//! every platform.

use std::fs;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

mod common;
use common::{CliHarness, reference_fixture};

/// A tiny valid reference-corpus file (test policy: reuse reference
/// fixtures, never write ad hoc CHAT).
const SMALL_FIXTURE: &str = "corpus/reference/edge-cases/empty-and-minimal.cha";

/// Resolve the fixture path relative to the workspace root.
fn fixture_path() -> std::path::PathBuf {
    reference_fixture(SMALL_FIXTURE)
}

/// With `TALKBANK_CHAT_CACHE_DIR` set, a validation run must create the
/// cache database inside that directory and nowhere else.
#[test]
fn cache_dir_env_override_is_honored() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let cache_root = tempdir()?;

    harness
        .chatter_cmd()
        .env("TALKBANK_CHAT_CACHE_DIR", cache_root.path())
        .arg("validate")
        .arg(fixture_path())
        .arg("--force")
        .assert()
        .success();

    let db = cache_root.path().join("talkbank-cache.db");
    assert!(
        db.exists(),
        "cache db not created under TALKBANK_CHAT_CACHE_DIR; \
         directory contents: {:?}",
        fs::read_dir(cache_root.path()).map(|d| d
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .collect::<Vec<_>>())
    );
    Ok(())
}
