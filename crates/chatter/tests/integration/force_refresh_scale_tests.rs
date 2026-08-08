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

//! `validate --force` at corpus scale, through the real CLI.
//!
//! Regression pin for the v0.5.0 DOA (2026-07-30): `--force` cleared the
//! cache by calling `clear_prefix` once per RESOLVED FILE, and each call
//! ran a full-table `SELECT DISTINCT file_path` plus a per-file DELETE
//! loop, so a corpus-sized invocation did O(n^2) work inside cache
//! initialization, BEFORE the progress display started. On the operator's
//! real cache (136k files) `chatter validate --force <corpus-root>` sat at
//! 150% CPU with a blank screen indefinitely. The corpus differential
//! never sees this because it always starts from an empty isolated cache;
//! this test warms a real cache through the real binary first.

use std::time::{Duration, Instant};

use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

use crate::common::{CliHarness, assert_success, write_fixture};

/// A minimal valid CHAT file (validates clean, so the warm pass caches
/// a verdict for every file).
const TINY_VALID_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
    @Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n\
    *CHI:\thello .\n@End\n";

/// `validate --force` over a corpus-sized directory must spend its time
/// VALIDATING, not clearing: the cache-refresh phase has to be linear and
/// batched. The quadratic per-file `clear_prefix` implementation exceeds
/// this bound by an order of magnitude at this size (and by hours at real
/// corpus size); the batched implementation clears in well under a second,
/// so the whole run is dominated by parsing 6,000 tiny files.
/// Calibrated 2026-07-30 on ming: old code 34.0s, validation alone ~2s.
#[test]
fn force_refresh_scales_to_corpus_sized_input() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir().map_err(|e| TestError::Failure(format!("tempdir: {e}")))?;
    for i in 0..6_000 {
        write_fixture(dir.path(), &format!("f{i:05}.cha"), TINY_VALID_CHAT)?;
    }

    // Warm pass: populates one cache entry per file through the real CLI.
    let warm = harness.run_validate(dir.path(), &[])?;
    assert_success(&warm, "warm validate over generated corpus");

    // Forced pass: clears every warmed entry, then revalidates.
    let started = Instant::now();
    let forced = harness.run_validate(dir.path(), &["--force"])?;
    let elapsed = started.elapsed();
    assert_success(&forced, "forced validate over generated corpus");

    assert!(
        elapsed < Duration::from_secs(15),
        "validate --force took {elapsed:?} for 6000 warmed files; the \
         cache-refresh phase is doing superlinear work again (v0.5.0 DOA class)"
    );
    Ok(())
}
