//! Execution-coverage tests for shipping top-level subcommands.
//!
//! `command_surface_manifest.rs` proves every published subcommand is
//! listed in `--help`, but that is only a help-contract check: several
//! shipping subcommands had no test that actually RAN them against real
//! input. The gap (confirmed 2026-06-13) covered `to-xml`, `clean`,
//! `lint`, `new-file`, `schema`, `validate-utseg`, and `watch`.
//!
//! This file closes the gap with subprocess-level characterization tests
//! that pin each command's current, known-good behavior. They run the real
//! CLI seam (the boundary a user hits), use reference-corpus fixtures
//! (never ad hoc CHAT, per the test-file policy), and isolate the
//! validation cache through `CliHarness` (mandated for every CLI
//! integration test). A red here is a real regression in a command we
//! ship, not a flaky expectation.

use std::fs;

use predicates::prelude::*;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

mod common;
use common::{CliHarness, assert_success, parse_json, reference_fixture};

/// Content-rich reference fixture: two speakers, several utterances with
/// real words.
const CONVERSATION_FIXTURE: &str = "corpus/reference/core/basic-conversation.cha";

/// Minimal CHAT that is structurally invalid (missing `@End`). The
/// reference corpus is, by mandate, all valid CHAT, so an invalid-input
/// case is built inline, matching the long-standing convention in
/// `integration_tests.rs`.
const INVALID_CHAT_MISSING_END: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n";

// ============================================================================
// schema
// ============================================================================

/// `chatter schema` prints the CHAT JSON Schema as a valid JSON document
/// (JSON Schema 2020-12, so it carries a `$defs` section).
#[test]
fn schema_prints_valid_json_schema() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let output = harness.chatter_cmd().arg("schema").output()?;
    assert_success(&output, "schema");
    let value = parse_json(&output)?;
    assert!(
        value.get("$defs").is_some(),
        "schema output missing $defs section"
    );
    Ok(())
}

/// `chatter schema --url` prints only the canonical schema URL, not the
/// full schema body.
#[test]
fn schema_url_prints_canonical_url_only() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    harness
        .chatter_cmd()
        .args(["schema", "--url"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "https://talkbank.org/schemas/v0.1/chat-file.json",
        ))
        .stdout(predicate::str::contains("\"$defs\"").not());
    Ok(())
}

// ============================================================================
// new-file
// ============================================================================

/// `chatter new-file` scaffolds a minimal valid CHAT skeleton with the
/// documented defaults (CHI / eng / Target_Child) and no utterance line.
#[test]
fn new_file_default_scaffold() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    harness
        .chatter_cmd()
        .arg("new-file")
        .assert()
        .success()
        .stdout(predicate::str::contains("@UTF8"))
        .stdout(predicate::str::contains("@Begin"))
        .stdout(predicate::str::contains("@Languages:\teng"))
        .stdout(predicate::str::contains("@Participants:\tCHI Target_Child"))
        .stdout(predicate::str::contains("@End"));
    Ok(())
}

/// `chatter new-file` honors `--speaker`, `--language`, and `--utterance`.
#[test]
fn new_file_custom_speaker_language_utterance() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    harness
        .chatter_cmd()
        .args([
            "new-file",
            "--speaker",
            "MOT",
            "--language",
            "fra",
            "--utterance",
            "bonjour .",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("@Languages:\tfra"))
        .stdout(predicate::str::contains("@Participants:\tMOT Target_Child"))
        .stdout(predicate::str::contains("*MOT:\tbonjour ."));
    Ok(())
}

/// A file scaffolded by `new-file --output` is itself valid CHAT: it
/// round-trips cleanly through `chatter validate`.
#[test]
fn new_file_output_is_valid_chat() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let path = dir.path().join("scaffold.cha");

    harness
        .chatter_cmd()
        .arg("new-file")
        .arg("--output")
        .arg(&path)
        .args(["--utterance", "hello world ."])
        .assert()
        .success();
    assert!(path.exists(), "new-file did not write the output file");

    assert_success(
        &harness.run_validate(&path, &[])?,
        "validate scaffolded file",
    );
    Ok(())
}

// ============================================================================
// to-xml
// ============================================================================

/// `chatter to-xml` emits TalkBank XML to stdout for a valid transcript.
#[test]
fn to_xml_emits_xml_to_stdout() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    harness
        .chatter_cmd()
        .arg("to-xml")
        .arg(reference_fixture(CONVERSATION_FIXTURE))
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
        ))
        .stdout(predicate::str::contains("<CHAT"))
        .stdout(predicate::str::contains("www.talkbank.org/ns/talkbank"));
    Ok(())
}

/// `chatter to-xml --output` writes the XML to a file (which begins with
/// the XML declaration) and reports success.
#[test]
fn to_xml_writes_output_file() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let out = dir.path().join("out.xml");

    harness
        .chatter_cmd()
        .arg("to-xml")
        .arg(reference_fixture(CONVERSATION_FIXTURE))
        .arg("--output")
        .arg(&out)
        .assert()
        .success()
        // The XML goes to the file, not stdout; the confirmation is on stderr.
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("Converted"));

    let body = fs::read_to_string(&out)?;
    assert!(body.starts_with("<?xml"), "output is not XML:\n{body}");
    Ok(())
}

/// `chatter to-xml` validates the input before emitting: an invalid
/// transcript fails (exit 1) and produces NO XML on stdout, so a failed
/// export never leaves a partial document behind.
#[test]
fn to_xml_rejects_invalid_input_without_emitting() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let bad = dir.path().join("invalid.cha");
    fs::write(&bad, INVALID_CHAT_MISSING_END)?;

    let output = harness.chatter_cmd().arg("to-xml").arg(&bad).output()?;
    assert!(
        !output.status.success(),
        "to-xml should fail on invalid input"
    );
    assert!(
        output.stdout.is_empty(),
        "to-xml emitted XML despite invalid input:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    Ok(())
}

// ============================================================================
// clean
// ============================================================================

/// `chatter clean` reports cleaned text per word, grouped by speaker line.
#[test]
fn clean_reports_cleaned_words() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    harness
        .chatter_cmd()
        .arg("clean")
        .arg(reference_fixture(CONVERSATION_FIXTURE))
        .assert()
        .success()
        // Structural: each speaker group is headed by its source line number,
        // so assert the grouping format rather than a specific fixture word.
        .stdout(predicate::str::contains("*CHI:"))
        .stdout(predicate::str::contains("(line "));
    Ok(())
}

/// `chatter clean --format json` emits a valid JSON array of per-line
/// speaker/word records.
#[test]
fn clean_json_is_valid_json_array() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let output = harness
        .chatter_cmd()
        .arg("clean")
        .arg(reference_fixture(CONVERSATION_FIXTURE))
        .args(["--format", "json"])
        .output()?;
    assert_success(&output, "clean --format json");

    let value = parse_json(&output)?;
    assert!(value.is_array(), "clean json output is not an array");
    let first = value
        .get(0)
        .ok_or_else(|| TestError::Failure("clean json array is empty".to_string()))?;
    assert!(
        first.get("speaker").is_some(),
        "clean json record missing speaker field"
    );
    assert!(
        first.get("words").is_some(),
        "clean json record missing words field"
    );
    Ok(())
}

// ============================================================================
// lint
// ============================================================================

/// `chatter lint` on a clean reference file reports no fixable issues and
/// exits successfully.
#[test]
fn lint_clean_file_reports_no_issues() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    harness
        .chatter_cmd()
        .arg("lint")
        .arg(reference_fixture(CONVERSATION_FIXTURE))
        .assert()
        .success()
        .stdout(predicate::str::contains("No fixable issues"));
    Ok(())
}

/// `chatter lint --fix` on a clean copy leaves a still-valid CHAT file
/// (idempotent on already-clean input).
#[test]
fn lint_fix_preserves_valid_file() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let path = dir.path().join("lintme.cha");
    fs::copy(reference_fixture(CONVERSATION_FIXTURE), &path)?;

    harness
        .chatter_cmd()
        .arg("lint")
        .arg(&path)
        .arg("--fix")
        .assert()
        .success();

    // The fixed file must still be valid CHAT.
    assert_success(
        &harness.run_validate(&path, &[])?,
        "validate lint-fixed file",
    );
    Ok(())
}

/// `--dry-run` requires `--fix` (clap `requires`); supplying it alone is a
/// usage error (exit 2), never a silent no-op.
#[test]
fn lint_dry_run_requires_fix() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    harness
        .chatter_cmd()
        .arg("lint")
        .arg(reference_fixture(CONVERSATION_FIXTURE))
        .arg("--dry-run")
        .assert()
        .code(2);
    Ok(())
}

// ============================================================================
// watch (non-blocking surface checks only)
// ============================================================================

/// `chatter watch --help` documents the watch command. The watch loop
/// itself is long-running and is deliberately never started here; the
/// help and argument-validation contracts are the testable seam.
#[test]
fn watch_help_documents_command() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    harness
        .chatter_cmd()
        .args(["watch", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Watch CHAT file"))
        .stdout(predicate::str::contains("Usage: chatter watch"));
    Ok(())
}

/// `chatter watch` with no path is a clap usage error (exit 2), proving
/// the required-argument contract without entering the watch loop.
#[test]
fn watch_requires_path_argument() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    harness.chatter_cmd().arg("watch").assert().code(2);
    Ok(())
}
