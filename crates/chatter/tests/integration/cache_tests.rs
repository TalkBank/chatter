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

//! Tests for validation cache functionality
//!
//! Note: Early placeholder tests have been replaced by full implementations below.
//! See test_cache_hit_performance, test_cache_invalidation_after_file_modification,
//! test_force_flag_clears_cache, and test_validate_single_file_cached_output.

use std::fs;
use std::thread;
use std::time::Duration;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

use crate::common::{CliHarness, assert_failure, assert_success, combined_output};

// Integration tests for the validation cache, exercised via CLI commands.

use std::path::Path;

/// Run `chatter validate` through the isolated CLI harness.
fn run_validate(
    harness: &CliHarness,
    path: &Path,
    extra_args: &[&str],
) -> Result<std::process::Output, TestError> {
    harness.run_validate(path, extra_args)
}

/// Tests validate command exists.
#[test]
fn test_validate_command_exists() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let output = harness.run_output(&["--help"])?;

    if !output.status.success() {
        return Err(TestError::Failure(
            "CLI should build successfully".to_string(),
        ));
    }
    Ok(())
}

/// Tests validate single file.
#[test]
fn test_validate_single_file() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("test.cha");

    // Create a valid CHAT file with required headers
    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    fs::write(&file_path, content)?;

    let output = run_validate(&harness, &file_path, &[])?;

    if !output.status.success() {
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }

    if !output.status.success() {
        return Err(TestError::Failure(
            "Valid file should pass validation".to_string(),
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("valid") && !stdout.contains("✓") {
        return Err(TestError::Failure(
            "Output should indicate file is valid".to_string(),
        ));
    }
    Ok(())
}

/// Tests validate invalid file.
#[test]
fn test_validate_invalid_file() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("invalid.cha");

    // Create an invalid CHAT file (missing @Begin)
    fs::write(&file_path, "*CHI:\thello .\n")?;

    let output = run_validate(&harness, &file_path, &[])?;

    if output.status.success() {
        return Err(TestError::Failure(
            "Invalid file should fail validation".to_string(),
        ));
    }
    Ok(())
}

/// Tests cache hit performance.
#[test]
fn test_cache_hit_performance() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("test.cha");

    // Create a valid CHAT file with required headers
    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    fs::write(&file_path, content)?;

    // First validation (cache miss)
    let start = std::time::Instant::now();
    let output1 = run_validate(&harness, &file_path, &[])?;
    let duration1 = start.elapsed();

    if !output1.status.success() {
        return Err(TestError::Failure(
            "First validation should succeed".to_string(),
        ));
    }

    // Second validation (should be cache hit)
    let start = std::time::Instant::now();
    let output2 = run_validate(&harness, &file_path, &[])?;
    let duration2 = start.elapsed();

    if !output2.status.success() {
        return Err(TestError::Failure(
            "Second validation should succeed".to_string(),
        ));
    }

    // Note: This is a weak test because cargo run has overhead
    // In a real scenario, second run should be much faster
    // For now, we just verify both succeed
    println!("First run: {:?}, Second run: {:?}", duration1, duration2);
    Ok(())
}

/// Tests cache invalidation after file modification.
#[test]
fn test_cache_invalidation_after_file_modification() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("test.cha");

    // Create initial file
    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    fs::write(&file_path, content)?;

    // First validation
    let output1 = run_validate(&harness, &file_path, &[])?;

    if !output1.status.success() {
        eprintln!("stdout: {}", String::from_utf8_lossy(&output1.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output1.stderr));
    }
    if !output1.status.success() {
        return Err(TestError::Failure(
            "Initial validation should succeed".to_string(),
        ));
    }

    // Wait a bit to ensure mtime changes
    thread::sleep(Duration::from_millis(100));

    // Modify file (change "hello world" to "goodbye")
    let modified_content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\tgoodbye .\n@End\n";
    fs::write(&file_path, modified_content)?;

    // Second validation (should detect modification and re-validate)
    let output2 = run_validate(&harness, &file_path, &[])?;
    if !output2.status.success() {
        return Err(TestError::Failure(
            "Second validation should succeed".to_string(),
        ));
    }

    // The cache should have been invalidated and file re-validated
    // We can't easily verify this without exposing cache internals,
    // but at least we know it doesn't crash
    Ok(())
}

/// Tests force flag clears cache.
#[test]
fn test_force_flag_clears_cache() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("test.cha");

    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    fs::write(&file_path, content)?;

    // First validate to populate cache
    let output1 = run_validate(&harness, &file_path, &[])?;
    if !output1.status.success() {
        return Err(TestError::Failure(
            "Initial validation should succeed".to_string(),
        ));
    }

    // Now validate with --force - should clear and re-validate
    let output2 = run_validate(&harness, &file_path, &["--force"])?;

    if !output2.status.success() {
        eprintln!("stdout: {}", String::from_utf8_lossy(&output2.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output2.stderr));
    }
    if !output2.status.success() {
        return Err(TestError::Failure(
            "Forced validation should succeed".to_string(),
        ));
    }
    let stderr = String::from_utf8_lossy(&output2.stderr);

    // When using --force, stderr should mention clearing cache entries
    if !stderr.contains("Cleared") || !stderr.contains("cache entries") {
        return Err(TestError::Failure(format!(
            "Output should indicate cache clearing when --force is used. Got stderr: {}",
            stderr
        )));
    }

    Ok(())
}

/// `--force` must clear the cache for EVERY input path, not only the first.
///
/// Regression: `initialize_validation_cache` cleared by the cosmetic
/// `summary_label` (the first input arg), so `chatter validate --force a.cha
/// b.cha` silently served b.cha's stale verdict. Found 2026-07-30 when a
/// gate-removal measurement over 994 CA files reported near-zero impact
/// because 993 of the verdicts came from the cache of the PREVIOUS build
/// (same rules fingerprint, changed rule behavior).
#[test]
fn test_force_flag_clears_cache_for_every_input_path() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    let file_a = dir.path().join("a.cha");
    let file_b = dir.path().join("b.cha");
    fs::write(&file_a, content)?;
    fs::write(&file_b, content)?;
    let a = file_a.to_string_lossy().to_string();
    let b = file_b.to_string_lossy().to_string();

    // Populate the cache for both files.
    let output1 = harness.run_output(&["validate", &a, &b])?;
    if !output1.status.success() {
        return Err(TestError::Failure(
            "initial validation should succeed".to_string(),
        ));
    }

    // Force-refresh both. Both cached entries must be cleared.
    let output2 = harness.run_output(&["validate", "--force", &a, &b])?;
    if !output2.status.success() {
        return Err(TestError::Failure(
            "forced validation should succeed".to_string(),
        ));
    }
    let stderr = String::from_utf8_lossy(&output2.stderr);
    if !stderr.contains("Cleared 2 cache entries") {
        return Err(TestError::Failure(format!(
            "--force must clear every input path's entries; stderr: {stderr}"
        )));
    }
    Ok(())
}

/// Tests validate directory with cache.
#[test]
fn test_validate_directory_with_cache() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;

    // Create multiple test files
    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";

    for i in 1..=5 {
        let file_path = dir.path().join(format!("test{}.cha", i));
        fs::write(&file_path, content)?;
    }

    // First directory validation (cache miss for all files)
    let output1 = run_validate(&harness, dir.path(), &[])?;

    if !output1.status.success() {
        return Err(TestError::Failure(
            "First directory validation should succeed".to_string(),
        ));
    }

    // Second directory validation (should use cache for all files)
    let output2 = run_validate(&harness, dir.path(), &[])?;

    if !output2.status.success() {
        return Err(TestError::Failure(
            "Second directory validation should succeed".to_string(),
        ));
    }

    // Both should report same results
    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    let stdout2 = String::from_utf8_lossy(&output2.stdout);

    if !(stdout1.contains("Valid: 5") || (stdout1.contains("5") && stdout1.contains("valid"))) {
        return Err(TestError::Failure(
            "Expected validation output for first run".to_string(),
        ));
    }
    if !(stdout2.contains("Valid: 5") || (stdout2.contains("5") && stdout2.contains("valid"))) {
        return Err(TestError::Failure(
            "Expected validation output for second run".to_string(),
        ));
    }
    Ok(())
}

/// Tests validate single file cached output.
#[test]
fn test_validate_single_file_cached_output() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("cached.cha");

    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    fs::write(&file_path, content)?;

    let first = run_validate(&harness, &file_path, &[])?;
    if !first.status.success() {
        return Err(TestError::Failure(
            "First validation should succeed".to_string(),
        ));
    }

    let second = run_validate(&harness, &file_path, &[])?;
    if !second.status.success() {
        return Err(TestError::Failure(
            "Second validation should succeed".to_string(),
        ));
    }
    let stdout = String::from_utf8_lossy(&second.stdout);
    if !stdout.contains("Cache hits: 1") {
        return Err(TestError::Failure(
            "Expected second validation run to report one cache hit".to_string(),
        ));
    }
    Ok(())
}

/// A minimal valid CHAT file, used by the cache-partitioning tests below.
const VALID_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";

/// A CHAT file whose only diagnostic is E370 (a retrace marker with nothing
/// after it), so `--suppress E370` empties its diagnostic set entirely.
const E370_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world [/] .\n@End\n";

/// Read the `Cache hits: N` figure out of a text-format run summary.
///
/// Asserting on COUNTED WORK rather than elapsed time is deliberate: every
/// cache regression this project has actually shipped was a change in how many
/// files got re-validated, which is an exact integer the summary already
/// prints, whereas a wall-clock threshold is machine-dependent and goes flaky.
fn cache_hits(output: &std::process::Output) -> Result<usize, TestError> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("Cache hits: "))
        .ok_or_else(|| TestError::Failure(format!("no `Cache hits:` line in summary:\n{stdout}")))?
        .trim()
        .parse::<usize>()
        .map_err(|error| TestError::Failure(format!("unparsable cache-hit count: {error}")))
}

/// Two validation runs that differ ONLY in `--suppress` must share one cache.
///
/// # The regression this pins (introduced in v0.6.0)
///
/// `--suppress` is a PRESENTATION preference: it changes which diagnostics the
/// user is shown, never which diagnostics the validator computes. v0.6.0 folded
/// the suppression set into the cache key, so every distinct `--suppress` list
/// got its own private cache and a second run over the ~106,000-file corpus
/// re-validated all of it from cold. The invariant is a count, not a duration:
/// on the second run every file must be a cache HIT.
#[test]
fn suppression_does_not_partition_the_validation_cache() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    for index in 1..=3 {
        fs::write(dir.path().join(format!("file{index}.cha")), VALID_CHAT)?;
    }

    let cold = run_validate(&harness, dir.path(), &[])?;
    assert_success(&cold, "cold validation run");
    if cache_hits(&cold)? != 0 {
        return Err(TestError::Failure(
            "the first run over an empty cache cannot hit anything".to_string(),
        ));
    }

    let suppressed = run_validate(&harness, dir.path(), &["--suppress", "E370"])?;
    assert_success(&suppressed, "suppressed validation run");
    let hits = cache_hits(&suppressed)?;
    if hits != 3 {
        return Err(TestError::Failure(format!(
            "a run differing only in --suppress must reuse the cache the first run \
             filled: expected 3 cache hits, got {hits}. A presentation preference is \
             in the cache key."
        )));
    }
    Ok(())
}

/// `--strict-linkers` genuinely changes WHAT IS COMPUTED (it turns on
/// E351-E355), so unlike `--suppress` it MUST partition the cache: a verdict
/// reached without those checks is not an answer for a run that wants them.
///
/// The companion to the test above: together they pin both directions, which
/// is what stops "share everything" being an acceptable fix for the
/// suppression regression.
#[test]
fn strict_linkers_does_partition_the_validation_cache() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    for index in 1..=3 {
        fs::write(dir.path().join(format!("file{index}.cha")), VALID_CHAT)?;
    }

    let lenient = run_validate(&harness, dir.path(), &[])?;
    assert_success(&lenient, "lenient validation run");

    let strict = run_validate(&harness, dir.path(), &["--strict-linkers"])?;
    assert_success(&strict, "strict-linkers validation run");
    let hits = cache_hits(&strict)?;
    if hits != 0 {
        return Err(TestError::Failure(format!(
            "--strict-linkers runs extra checks, so a lenient verdict must not be \
             served to it: expected 0 cache hits, got {hits}"
        )));
    }
    Ok(())
}

/// Suppression still suppresses: the code disappears from the report, and a
/// file whose only diagnostic was suppressed stops counting as invalid.
///
/// This is v0.6.0 behaviour and the fix must not regress it. It is the reason
/// the cached value is NOT a pass/fail verdict under the active policy but the
/// narrower fact "this file produced no diagnostics at all", which is the same
/// under every policy.
#[test]
fn suppressing_the_only_diagnostic_hides_it_and_clears_the_verdict() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    fs::write(dir.path().join("retrace.cha"), E370_CHAT)?;

    let unsuppressed = run_validate(&harness, dir.path(), &[])?;
    assert_failure(&unsuppressed, "a file with E370 must fail validation");
    if !combined_output(&unsuppressed).contains("E370") {
        return Err(TestError::Failure(
            "the unsuppressed run must report E370".to_string(),
        ));
    }

    let suppressed = run_validate(&harness, dir.path(), &["--suppress", "E370"])?;
    assert_success(&suppressed, "suppressed run over an E370-only file");
    let shown = combined_output(&suppressed);
    if shown.contains("error[E370]") {
        return Err(TestError::Failure(format!(
            "a suppressed code must not be reported:\n{shown}"
        )));
    }
    if !shown.contains("Invalid: 0") {
        return Err(TestError::Failure(format!(
            "a file whose every diagnostic is suppressed has nothing left to fail \
             on:\n{shown}"
        )));
    }
    Ok(())
}

/// Suppressing one code must not touch the verdict on files that have OTHER
/// diagnostics. The v0.6.0 defect this guards: post-hoc filtering adjusted the
/// tallies twice and reported `Invalid: 0` for a corpus with genuinely invalid
/// files, exiting 0.
#[test]
fn suppression_does_not_clear_other_files_verdicts() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    fs::write(dir.path().join("retrace.cha"), E370_CHAT)?;
    fs::write(dir.path().join("headerless.cha"), "*CHI:\thello .\n")?;

    let suppressed = run_validate(&harness, dir.path(), &["--suppress", "E370"])?;
    assert_failure(
        &suppressed,
        "a file with unsuppressed errors must still fail the run",
    );
    let shown = combined_output(&suppressed);
    if !shown.contains("Invalid: 1") {
        return Err(TestError::Failure(format!(
            "exactly the file with unsuppressed errors must count invalid:\n{shown}"
        )));
    }
    Ok(())
}

// Cache management command tests

/// Tests cache stats command.
#[test]
fn test_cache_stats_command() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("test.cha");

    // Create valid file
    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    fs::write(&file_path, content)?;

    // Validate to populate cache
    let validate = run_validate(&harness, &file_path, &[])?;
    if !validate.status.success() {
        return Err(TestError::Failure("Validation should succeed".to_string()));
    }

    // Run cache stats command
    let stats = harness.run_output(&["cache", "stats"])?;

    if !stats.status.success() {
        return Err(TestError::Failure("cache stats should succeed".to_string()));
    }
    let stdout = String::from_utf8_lossy(&stats.stdout);

    // Verify output contains expected sections
    if !stdout.contains("Cache Statistics") {
        return Err(TestError::Failure(
            "Missing Cache Statistics output".to_string(),
        ));
    }
    if !stdout.contains("Cache Directory:") {
        return Err(TestError::Failure(
            "Missing Cache Directory output".to_string(),
        ));
    }
    if !stdout.contains("Total Entries:") {
        return Err(TestError::Failure(
            "Missing Total Entries output".to_string(),
        ));
    }
    Ok(())
}

/// Tests cache clear dry run.
#[test]
fn test_cache_clear_dry_run() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("test.cha");

    // Create valid file
    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    fs::write(&file_path, content)?;

    // Validate to populate cache
    let validate = run_validate(&harness, &file_path, &[])?;
    if !validate.status.success() {
        return Err(TestError::Failure("Validation should succeed".to_string()));
    }

    // Run cache clear with dry-run
    let clear = harness.run_output(&[
        "cache",
        "clear",
        "--prefix",
        dir.path()
            .to_str()
            .ok_or_else(|| TestError::Failure("Invalid directory path".to_string()))?,
        "--dry-run",
    ])?;

    if !clear.status.success() {
        return Err(TestError::Failure(
            "cache clear --dry-run should succeed".to_string(),
        ));
    }
    let stdout = String::from_utf8_lossy(&clear.stdout);
    if !stdout.contains("Would clear") {
        return Err(TestError::Failure(
            "Expected dry-run to mention Would clear".to_string(),
        ));
    }
    if !stdout.contains("dry-run") {
        return Err(TestError::Failure(
            "Expected dry-run to mention dry-run".to_string(),
        ));
    }
    Ok(())
}

/// Tests cache clear prefix.
#[test]
fn test_cache_clear_prefix() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let file_path = dir.path().join("test.cha");

    // Create valid file
    let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n@End\n";
    fs::write(&file_path, content)?;

    // Validate to populate cache
    let validate = run_validate(&harness, &file_path, &[])?;
    if !validate.status.success() {
        return Err(TestError::Failure("Validation should succeed".to_string()));
    }

    // Clear cache for this prefix
    let clear = harness.run_output(&[
        "cache",
        "clear",
        "--prefix",
        dir.path()
            .to_str()
            .ok_or_else(|| TestError::Failure("Invalid directory path".to_string()))?,
    ])?;

    if !clear.status.success() {
        return Err(TestError::Failure("cache clear should succeed".to_string()));
    }
    let stdout = String::from_utf8_lossy(&clear.stdout);
    if !stdout.contains("Cleared") {
        return Err(TestError::Failure("Expected Cleared output".to_string()));
    }
    if !stdout.contains("cache entries") {
        return Err(TestError::Failure(
            "Expected cache entries output".to_string(),
        ));
    }
    Ok(())
}

/// Tests cache clear all.
#[test]
fn test_cache_clear_all() -> Result<(), TestError> {
    let harness = CliHarness::new()?;

    // Run cache clear --all
    let clear = harness.run_output(&["cache", "clear", "--all"])?;

    if !clear.status.success() {
        return Err(TestError::Failure(
            "cache clear --all should succeed".to_string(),
        ));
    }
    let stdout = String::from_utf8_lossy(&clear.stdout);
    if !stdout.contains("Cleared") {
        return Err(TestError::Failure("Expected Cleared output".to_string()));
    }
    Ok(())
}
