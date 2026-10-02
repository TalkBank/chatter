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

//! CLI integration tests
//!
//! These tests exercise the CLI commands end-to-end using assert_cmd.

use predicates::prelude::*;
use std::fs;
use std::path::Path;
use talkbank_parser_tests::test_error::TestError;
use tempfile::{NamedTempFile, tempdir};

// ============================================================================
// Test Fixtures
// ============================================================================

const VALID_CHAT: &str = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	hello world .
%mor:	n|hello n|world .
@End
"#;

const INVALID_CHAT_MISSING_END: &str = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello .
"#;

const INVALID_CHAT_SYNTAX_ERROR: &str = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello@ world .
@End
"#;

const CHAT_WITH_ALIGNMENT_ERROR: &str = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	I want cookie .
%mor:	pro|I v|want .
@Comment:	ERROR: Missing n|cookie in %mor
@End
"#;

// A file intended to carry ONLY warning-severity diagnostics. Valid CHAT:
// warnings do not make a file invalid, so its per-file headline must be
// advisory, not an error. NOTE: this fixture rode on the E254
// undeclared-@s:CODE warning, retired 2026-07-15 (an undeclared word-level
// language override is now silently valid), and no default-config construct
// currently produces a warning-severity diagnostic through `chatter
// validate`, so the subprocess test below is ignored until one exists again
// (candidate: the W601/W602 severity-taxonomy adjudication). The headline
// logic itself is pinned by unit tests on `has_hard_error` in
// `src/output.rs`.
const WARNINGS_ONLY_CHAT: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:\thello hola@s:spa .
@End
";

// ============================================================================
// Validate Command Tests
// ============================================================================

/// Tests validate valid file.
#[test]
fn test_validate_valid_file() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("valid.cha");
    fs::write(&file_path, VALID_CHAT)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Total files: 1"))
        .stdout(predicate::str::contains("Valid: 1"))
        .stdout(predicate::str::contains("Invalid: 0"));
    Ok(())
}

/// Tests validate invalid file missing end.
#[test]
fn test_validate_invalid_file_missing_end() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("invalid.cha");
    fs::write(&file_path, INVALID_CHAT_MISSING_END)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .assert()
        .failure();
    Ok(())
}

/// Multi-file `validate` must process EVERY file passed on the command
/// line, even when an earlier file in the list is invalid. The early-exit
/// regression (fixed 2026-05-03) caused `chatter validate bad.cha good.cha`
/// to terminate after `bad.cha` and never visit `good.cha`.
#[test]
fn test_validate_multi_file_processes_every_argument() -> Result<(), TestError> {
    let dir = tempdir()?;
    let bad = dir.path().join("aa_invalid.cha");
    let good = dir.path().join("zz_valid.cha");
    fs::write(&bad, INVALID_CHAT_MISSING_END)?;
    fs::write(&good, VALID_CHAT)?;

    // Process exits non-zero (the bad file is invalid) and the summary
    // accounts for both files, proving the run did not bail out after the
    // first failure.
    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&bad)
        .arg(&good)
        .assert()
        .failure()
        .stderr(predicate::str::contains("aa_invalid.cha"))
        .stdout(predicate::str::contains("Total files: 2"))
        .stdout(predicate::str::contains("Valid: 1"))
        .stdout(predicate::str::contains("Invalid: 1"));
    Ok(())
}

/// Tests text-mode validation keeps human diagnostics on stderr.
#[test]
fn test_validate_invalid_file_text_mode_uses_stderr_for_diagnostics() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("invalid.cha");
    fs::write(&file_path, INVALID_CHAT_MISSING_END)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .assert()
        .failure()
        .stdout(predicate::str::contains("Total files: 1"))
        .stdout(predicate::str::contains("Invalid: 1"))
        .stderr(predicate::str::contains("✗ Errors found in"))
        .stderr(predicate::str::contains("invalid.cha"))
        .stderr(predicate::str::contains(
            "Missing @End header at end of file",
        ));
    Ok(())
}

/// A warnings-only file is valid CHAT, so it must NOT be headlined as an error.
///
/// Regression for the per-file headline keying on "has any diagnostic" instead
/// of "has any hard error": a file whose sole diagnostic is a warning was
/// printed as `✗ Errors found in <file>` and given the "fix the structural
/// errors first" cascading hint, while the summary correctly reported it Valid.
/// The headline must instead read `⚠ Warnings in <file>`, the cascading hint
/// (which is about hard structural errors tainting the parse) must not fire,
/// and the run must succeed with `Valid: 1`.
#[test]
#[ignore = "no default-config construct currently produces a warning-severity \
            diagnostic (E254 retired 2026-07-15); re-enable with a real \
            warnings-only fixture once one exists (see WARNINGS_ONLY_CHAT \
            note). The headline logic is unit-pinned in src/output.rs."]
fn test_validate_warnings_only_file_not_headlined_as_error() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("warnings_only.cha");
    fs::write(&file_path, WARNINGS_ONLY_CHAT)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Valid: 1"))
        .stdout(predicate::str::contains("Invalid: 0"))
        // The warning itself must still be shown to the user.
        .stderr(predicate::str::contains("⚠ Warnings in"))
        .stderr(predicate::str::contains("warnings_only.cha"))
        // But it must NOT be framed as an error, nor pointed at nonexistent
        // structural errors.
        .stderr(predicate::str::contains("✗ Errors found in").not())
        .stderr(predicate::str::contains("Fix the structural errors first").not());
    Ok(())
}

/// Tests validate invalid file syntax error.
#[test]
fn test_validate_invalid_file_syntax_error() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("syntax_error.cha");
    fs::write(&file_path, INVALID_CHAT_SYNTAX_ERROR)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .assert()
        .failure();
    Ok(())
}

/// Tests JSON validation keeps machine-readable output off stderr.
#[test]
fn test_validate_invalid_file_json_mode_keeps_stderr_clean() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("invalid.cha");
    fs::write(&file_path, INVALID_CHAT_MISSING_END)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg("--format")
        .arg("json")
        .arg(&file_path)
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"status\":\"invalid\""))
        .stdout(predicate::str::contains("\"type\":\"summary\""))
        .stdout(predicate::str::contains("\"file\":\""))
        .stdout(predicate::str::contains(
            "Missing @End header at end of file",
        ))
        .stderr(predicate::str::is_empty());
    Ok(())
}

/// Cache housekeeping notes must not reach stderr in JSON mode.
///
/// BOUNDARY test, and it has to be: the defect is in what the process writes to
/// a file descriptor, which no signature can describe.
///
/// `--force` makes this deterministic. The neighbouring test above found the
/// same defect by accident and only under parallel load, because it depends on
/// a cache PRUNE happening to fire, which needs superseded rows: shipped
/// 0.11.0 emitted `note: pruned 1 unreachable cache row(s)...` there roughly
/// one run in many. `--force` takes the other branch of the same function every
/// time, and shipped 0.11.0 writes exactly 24 bytes of `Cleared 0 cache
/// entries` to a stream the JSON contract says is empty.
///
/// The routing is now a type (`CacheNotices`) rather than a bare `eprintln!`,
/// so this test guards the CALLERS' choice of variant, which a type cannot
/// make for them.
#[test]
fn cache_housekeeping_notes_stay_off_stderr_in_json_mode() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("valid.cha");
    fs::write(&file_path, VALID_CHAT)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg("--format")
        .arg("json")
        .arg("--force")
        .arg(&file_path)
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
    Ok(())
}

/// Tests validate file not found.
#[test]
fn test_validate_file_not_found() {
    crate::common::chatter_cmd()
        .arg("validate")
        .arg("/nonexistent/file.cha")
        .assert()
        .failure();
}

/// Tests validate quiet mode success.
#[test]
fn test_validate_quiet_mode_success() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("valid.cha");
    fs::write(&file_path, VALID_CHAT)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .arg("--quiet")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().or(predicate::str::contains("valid.cha")));
    Ok(())
}

/// Tests validate quiet mode failure.
#[test]
fn test_validate_quiet_mode_failure() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("invalid.cha");
    fs::write(&file_path, INVALID_CHAT_MISSING_END)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .arg("--quiet")
        .assert()
        .failure();
    Ok(())
}

/// Tests validate skip alignment.
#[test]
fn test_validate_skip_alignment() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("alignment_issue.cha");
    fs::write(&file_path, CHAT_WITH_ALIGNMENT_ERROR)?;

    // With alignment checking (default): should detect error
    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .assert()
        .failure();

    // With --skip-alignment: should pass (only validates structure)
    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .arg("--skip-alignment")
        .assert()
        .success();
    Ok(())
}

/// Tests validate directory recursive.
#[test]
fn test_validate_directory_recursive() -> Result<(), TestError> {
    let dir = tempdir()?;

    // Create files in nested structure
    fs::write(dir.path().join("root.cha"), VALID_CHAT)?;

    let subdir = dir.path().join("subdir");
    fs::create_dir(&subdir)?;
    fs::write(subdir.join("nested.cha"), VALID_CHAT)?;

    // Directories are always validated recursively by default
    crate::common::chatter_cmd()
        .arg("validate")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Total files: 2")); // Both files validated
    Ok(())
}

/// Directory validation ignores macOS AppleDouble sidecars such as `._file.cha`.
#[test]
fn test_validate_directory_ignores_appledouble_sidecars() -> Result<(), TestError> {
    let dir = tempdir()?;

    fs::write(dir.path().join("real.cha"), VALID_CHAT)?;
    fs::write(dir.path().join("._real.cha"), b"\0\x05AppleDouble metadata")?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(dir.path())
        .arg("--force")
        .arg("--tui-mode")
        .arg("disable")
        .assert()
        .success()
        .stdout(predicate::str::contains("Total files: 1"))
        .stdout(predicate::str::contains("Invalid: 0"))
        .stderr(predicate::str::contains("read error").not());
    Ok(())
}

/// Tests validate json output.
#[test]
fn test_validate_json_output() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("valid.cha");
    fs::write(&file_path, VALID_CHAT)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("{"));
    Ok(())
}

// ============================================================================
// Normalize Command Tests
// ============================================================================

/// Tests normalize to stdout.
#[test]
fn test_normalize_to_stdout() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("input.cha");
    fs::write(&file_path, VALID_CHAT)?;

    crate::common::chatter_cmd()
        .arg("normalize")
        .arg(&file_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("@UTF8"))
        .stdout(predicate::str::contains("*CHI:"));
    Ok(())
}

/// Tests normalize to file.
#[test]
fn test_normalize_to_file() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_path = dir.path().join("input.cha");
    let output_path = dir.path().join("output.cha");

    fs::write(&input_path, VALID_CHAT)?;

    crate::common::chatter_cmd()
        .arg("normalize")
        .arg(&input_path)
        .arg("--output")
        .arg(&output_path)
        .assert()
        .success();

    // Verify output file was created
    if !output_path.exists() {
        return Err(TestError::Failure("Expected output file".to_string()));
    }
    let content = fs::read_to_string(&output_path)?;
    if !content.contains("@UTF8") || !content.contains("*CHI:") {
        return Err(TestError::Failure(
            "Normalized output missing expected headers".to_string(),
        ));
    }
    Ok(())
}

/// Tests normalize with validation.
#[test]
fn test_normalize_with_validation() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("invalid.cha");
    fs::write(&file_path, INVALID_CHAT_MISSING_END)?;

    crate::common::chatter_cmd()
        .arg("normalize")
        .arg(&file_path)
        .arg("--validate")
        .assert()
        .failure();
    Ok(())
}

// ============================================================================
// ToJson Command Tests
// ============================================================================

/// Tests to json stdout.
#[test]
fn test_to_json_stdout() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("input.cha");
    fs::write(&file_path, VALID_CHAT)?;

    crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&file_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("{"))
        .stdout(predicate::str::contains("lines"));
    Ok(())
}

/// Tests to json file.
#[test]
fn test_to_json_file() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_path = dir.path().join("input.cha");
    let output_path = dir.path().join("output.json");

    fs::write(&input_path, VALID_CHAT)?;

    crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&input_path)
        .arg("--output")
        .arg(&output_path)
        .assert()
        .success();

    // Verify JSON file was created and is valid
    if !output_path.exists() {
        return Err(TestError::Failure("Expected JSON output file".to_string()));
    }
    let content = fs::read_to_string(&output_path)?;
    let _: serde_json::Value = serde_json::from_str(&content)
        .map_err(|err| TestError::Failure(format!("Output should be valid JSON: {err}")))?;
    Ok(())
}

/// Tests to json pretty vs compact.
#[test]
fn test_to_json_pretty_vs_compact() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("input.cha");
    fs::write(&file_path, VALID_CHAT)?;

    // Pretty (default)
    let pretty_output = crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&file_path)
        .output()?;

    let pretty_json = String::from_utf8(pretty_output.stdout)
        .map_err(|err| TestError::Failure(format!("Invalid UTF-8: {err}")))?;

    // Compact
    let compact_output = crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&file_path)
        .arg("--compact")
        .output()?;

    let compact_json = String::from_utf8(compact_output.stdout)
        .map_err(|err| TestError::Failure(format!("Invalid UTF-8: {err}")))?;

    // Pretty should have more whitespace
    if pretty_json.len() <= compact_json.len() {
        return Err(TestError::Failure(
            "Pretty JSON should be longer than compact JSON".to_string(),
        ));
    }
    if !pretty_json.contains("  ") {
        return Err(TestError::Failure(
            "Pretty JSON should contain indentation".to_string(),
        ));
    }
    Ok(())
}

/// Tests to json with validation.
#[test]
fn test_to_json_with_validation() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("invalid.cha");
    fs::write(&file_path, INVALID_CHAT_MISSING_END)?;

    crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&file_path)
        .arg("--validate")
        .assert()
        .failure();
    Ok(())
}

/// `--max-errors` says it stopped the run only when the limit stopped it: a
/// run that reaches the limit says so, and a clean run under the same limit
/// never does.
#[test]
fn test_max_errors_reports_a_stop_only_when_the_limit_stops_the_run() -> Result<(), TestError> {
    let dir = tempdir()?;
    let invalid = dir.path().join("invalid");
    let clean = dir.path().join("clean");
    fs::create_dir_all(&invalid)?;
    fs::create_dir_all(&clean)?;
    for name in ["a", "b", "c"] {
        fs::write(
            invalid.join(format!("{name}.cha")),
            INVALID_CHAT_MISSING_END,
        )?;
        fs::write(clean.join(format!("{name}.cha")), VALID_CHAT)?;
    }
    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&invalid)
        .args(["--max-errors", "1", "--jobs", "1"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "Stopped after reaching the error limit (1)",
        ));
    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&clean)
        .args(["--max-errors", "1"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Stopped after").not());
    // JSON mode says it as a record on stdout and keeps stderr empty, as its
    // contract promises.
    let output = crate::common::chatter_cmd()
        .arg("validate")
        .arg(&invalid)
        .args(["--max-errors", "1", "--jobs", "1", "--format", "json"])
        .output()?;
    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stop_records: Vec<serde_json::Value> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|record| record["type"] == "stop")
        .collect();
    assert_eq!(
        stop_records,
        [serde_json::json!({
            "type": "stop",
            "reason": "max_errors",
            "limit": 1,
            "unprocessed_files": 2
        })]
    );
    Ok(())
}

/// Run `validate --format json` with `args` and return its exit code and
/// its stop records. JSON mode keeps stderr empty, so that is asserted too.
fn json_stop_records(
    target: &Path,
    args: &[&str],
) -> Result<(Option<i32>, Vec<serde_json::Value>), TestError> {
    let output = crate::common::chatter_cmd()
        .arg("validate")
        .arg(target)
        .args(args)
        .args(["--format", "json"])
        .output()?;
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stops = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|record| record["type"] == "stop")
        .collect();
    Ok((output.status.code(), stops))
}

/// `--max-errors` counts errors, never warnings: a warning-only transcript
/// (W110, a case-only `@Media` difference) under a limit of 1 is validated,
/// nothing stops, and the run passes.
#[test]
fn max_errors_counts_errors_not_warnings() -> Result<(), TestError> {
    let dir = tempdir()?;
    fs::write(
        dir.path().join("Session.cha"),
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
         @ID:\teng|corpus|CHI|||||Target_Child|||\n@Media:\tsession, audio\n\
         *CHI:\thello .\u{15}0_1500\u{15}\n@End\n",
    )?;
    let (code, stops) = json_stop_records(dir.path(), &["--max-errors", "1", "--force"])?;
    assert_eq!(
        stops,
        Vec::<serde_json::Value>::new(),
        "a warning stopped the run"
    );
    assert_eq!(code, Some(0), "a warning-only corpus is valid");
    Ok(())
}

/// A file with warnings and no error is ONE `valid` record carrying its
/// warnings, and the run passes.
#[test]
fn json_mode_gives_a_warnings_only_file_one_valid_record() -> Result<(), TestError> {
    let dir = tempdir()?;
    fs::write(
        dir.path().join("Session.cha"),
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
         @ID:\teng|corpus|CHI|||||Target_Child|||\n@Media:\tsession, audio\n\
         *CHI:\thello .\u{15}0_1500\u{15}\n@End\n",
    )?;
    let output = crate::common::chatter_cmd()
        .arg("validate")
        .arg(dir.path())
        .args(["--format", "json", "--force"])
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "a warning-only corpus is valid"
    );
    let files: Vec<serde_json::Value> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|record| record["type"] == "file")
        .collect();
    assert_eq!(files.len(), 1, "one record per file: {files:?}");
    assert_eq!(files[0]["status"], "valid");
    assert_eq!(files[0]["warnings"][0]["code"], "W110");
    assert_eq!(files[0]["warnings"][0]["severity"], "Warning");
    Ok(())
}

/// A JSON consumer that closes its pipe ends the stream: the run exits 1,
/// stderr stays empty, and nothing panics.
#[test]
fn json_mode_with_a_closed_stdout_fails_without_a_panic() -> Result<(), TestError> {
    let dir = tempdir()?;
    for name in ["a", "b", "c"] {
        fs::write(dir.path().join(format!("{name}.cha")), VALID_CHAT)?;
    }
    let cache = tempdir()?;
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_chatter"))
        .env("TALKBANK_CHAT_CACHE_DIR", cache.path())
        .arg("validate")
        .arg(dir.path())
        .args(["--format", "json", "--force"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    // The reading end goes before the run can write its summary.
    drop(child.stdout.take());
    let output = child.wait_with_output()?;
    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// A limit reached by the run's last file stops nothing, so it is not
/// reported as a stop: the run covered every file, and the stop is read off
/// how the run ended.
#[test]
fn max_errors_reached_by_the_last_file_is_not_a_stop() -> Result<(), TestError> {
    let dir = tempdir()?;
    fs::write(dir.path().join("only.cha"), INVALID_CHAT_MISSING_END)?;
    let (code, stops) = json_stop_records(dir.path(), &["--max-errors", "1"])?;
    assert_eq!(
        stops,
        Vec::<serde_json::Value>::new(),
        "nothing was stopped"
    );
    assert_eq!(code, Some(1), "the one file is invalid");
    Ok(())
}

/// JSON mode keeps stderr empty by construction: notes about the run
/// (`--suppress`, the deprecated `--check-xphon`) are `notice` records, and
/// an input that cannot be read is a `read_error` record in the run's own
/// results that fails it.
#[test]
fn json_mode_says_everything_on_stdout() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file = dir.path().join("a.cha");
    fs::write(&file, VALID_CHAT)?;
    let missing = dir.path().join("missing.cha");
    let records =
        |args: &[&std::ffi::OsStr]| -> Result<(Option<i32>, Vec<serde_json::Value>), TestError> {
            let output = crate::common::chatter_cmd()
                .arg("validate")
                .args(args)
                .args(["--format", "json"])
                .output()?;
            assert!(
                output.stderr.is_empty(),
                "{args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            Ok((
                output.status.code(),
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .filter_map(|line| serde_json::from_str(line).ok())
                    .collect(),
            ))
        };
    let (code, out) = records(&[file.as_os_str(), "--suppress".as_ref(), "E736".as_ref()])?;
    assert_eq!(code, Some(0));
    assert!(
        out.iter()
            .any(|r| r["type"] == "notice" && r["notice"] == "suppressing")
    );
    let (code, out) = records(&[file.as_os_str(), "--check-xphon".as_ref()])?;
    assert_eq!(code, Some(0));
    assert!(
        out.iter()
            .any(|r| r["type"] == "notice" && r["flag"] == "--check-xphon")
    );
    let (code, out) = records(&[missing.as_os_str()])?;
    assert_eq!(code, Some(1), "an unreadable input fails the run");
    assert!(
        out.iter().any(|r| r["status"] == "read_error"),
        "the unreadable input is a record: {out:?}"
    );
    Ok(())
}

/// A transcript that does not parse prints its diagnostics on every to-json
/// path: one file or a directory, with CHAT validation on or skipped. The
/// single-file path used to print only a one-line summary for a parse
/// failure, because its error match ended in a catch-all.
#[test]
fn test_to_json_prints_parse_diagnostics_on_every_path() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_dir = dir.path().join("corpus");
    fs::create_dir_all(&input_dir)?;
    let file_path = input_dir.join("broken.cha");
    fs::write(&file_path, INVALID_CHAT_SYNTAX_ERROR)?;
    let output_dir = dir.path().join("json");

    for checks in [&[][..], &["--skip-validation"][..]] {
        crate::common::chatter_cmd()
            .arg("to-json")
            .arg(&file_path)
            .args(checks)
            .assert()
            .code(1)
            .stderr(predicate::str::contains("Errors found in"));
        crate::common::chatter_cmd()
            .arg("to-json")
            .arg(&input_dir)
            .arg("--output-dir")
            .arg(&output_dir)
            .args(checks)
            .assert()
            .code(1)
            .stderr(predicate::str::contains("Errors found in"));
    }
    Ok(())
}

// ============================================================================
// ToJson Directory Mode Tests
// ============================================================================

/// Tests to-json directory mode creates JSON files preserving structure.
#[test]
fn test_to_json_directory_mode() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_dir = dir.path().join("corpus");
    let sub_dir = input_dir.join("sub");
    fs::create_dir_all(&sub_dir)?;
    fs::write(input_dir.join("a.cha"), VALID_CHAT)?;
    fs::write(sub_dir.join("b.cha"), VALID_CHAT)?;

    let output_dir = dir.path().join("json");

    crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--skip-validation")
        .arg("--skip-schema-validation")
        .assert()
        .success()
        .stderr(predicate::str::contains("2 converted"));

    // Verify structure preserved
    assert!(output_dir.join("a.json").exists());
    assert!(output_dir.join("sub/b.json").exists());

    // Verify valid JSON
    let content = fs::read_to_string(output_dir.join("a.json"))?;
    let _: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| TestError::Failure(format!("Invalid JSON: {e}")))?;
    Ok(())
}

/// Tests incremental mode skips up-to-date files.
#[test]
fn test_to_json_incremental() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_dir = dir.path().join("corpus");
    fs::create_dir_all(&input_dir)?;
    fs::write(input_dir.join("a.cha"), VALID_CHAT)?;

    let output_dir = dir.path().join("json");

    // First run: should convert
    crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--skip-validation")
        .arg("--skip-schema-validation")
        .assert()
        .success()
        .stderr(predicate::str::contains("1 converted, 0 up-to-date"));

    // Second run: should skip (up-to-date)
    crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--skip-validation")
        .arg("--skip-schema-validation")
        .assert()
        .success()
        .stderr(predicate::str::contains("0 converted, 1 up-to-date"));
    Ok(())
}

/// Tests --force ignores mtime and reconverts all files.
#[test]
fn test_to_json_force() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_dir = dir.path().join("corpus");
    fs::create_dir_all(&input_dir)?;
    fs::write(input_dir.join("a.cha"), VALID_CHAT)?;

    let output_dir = dir.path().join("json");

    // First run
    crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--skip-validation")
        .arg("--skip-schema-validation")
        .assert()
        .success();

    // Force run: should reconvert despite up-to-date
    crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--skip-validation")
        .arg("--skip-schema-validation")
        .arg("--force")
        .assert()
        .success()
        .stderr(predicate::str::contains("1 converted, 0 up-to-date"));
    Ok(())
}

/// Every file is counted exactly once, in the right bucket: converted,
/// up-to-date or failed. The failure is a file that is not UTF-8, so it fails
/// at the read without needing an invalid CHAT fixture, and it makes the run
/// exit 1. The pool width is not varied here: serial and parallel runs share
/// one pool, whose distribution is tested in `talkbank_transform::worker_pool`.
#[test]
fn test_to_json_counts_every_file_once() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_dir = dir.path().join("corpus");
    let sub_dir = input_dir.join("sub");
    fs::create_dir_all(&sub_dir)?;
    for name in ["a", "b", "c"] {
        fs::write(input_dir.join(format!("{name}.cha")), VALID_CHAT)?;
        fs::write(sub_dir.join(format!("{name}.cha")), VALID_CHAT)?;
    }
    fs::write(input_dir.join("not-utf8.cha"), [0xff_u8, 0xfe, 0xfd])?;
    let output_dir = dir.path().join("json");

    let run = || {
        crate::common::chatter_cmd()
            .arg("to-json")
            .arg(&input_dir)
            .arg("--output-dir")
            .arg(&output_dir)
            .arg("--skip-validation")
            .arg("--skip-schema-validation")
            .args(["--jobs", "3"])
            .assert()
            .code(1)
    };
    run().stderr(predicate::str::contains(
        "Done: 6 converted, 0 up-to-date, 1 failed",
    ));
    for name in ["a", "b", "c"] {
        assert!(output_dir.join(format!("{name}.json")).exists());
        assert!(output_dir.join(format!("sub/{name}.json")).exists());
    }
    run().stderr(predicate::str::contains(
        "Done: 0 converted, 6 up-to-date, 1 failed",
    ));
    Ok(())
}

/// A directory the walk cannot read is reported and refuses the run before
/// anything is converted; it is not a smaller corpus that converted cleanly.
#[cfg(unix)]
#[test]
fn test_to_json_reports_an_unreadable_directory() -> Result<(), TestError> {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempdir()?;
    let input_dir = dir.path().join("corpus");
    let locked = input_dir.join("locked");
    fs::create_dir_all(&locked)?;
    fs::write(input_dir.join("a.cha"), VALID_CHAT)?;
    fs::write(locked.join("b.cha"), VALID_CHAT)?;
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000))?;
    // A process that can read the directory anyway (root) cannot run this
    // test; say so rather than pass without testing anything.
    let readable_anyway = fs::read_dir(&locked).is_ok();
    let output_dir = dir.path().join("json");
    let output = crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--skip-validation")
        .arg("--skip-schema-validation")
        .output();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755))?;
    let output = output?;
    assert!(
        !readable_anyway,
        "a mode 000 directory was readable: run this test as an ordinary user, not root"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("cannot read"), "{stderr}");
    assert!(stderr.contains("nothing was processed"), "{stderr}");
    assert!(!output_dir.join("a.json").exists(), "nothing is converted");
    Ok(())
}

/// Tests --prune removes orphaned .json files.
#[test]
fn test_to_json_prune() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_dir = dir.path().join("corpus");
    fs::create_dir_all(&input_dir)?;
    fs::write(input_dir.join("a.cha"), VALID_CHAT)?;

    let output_dir = dir.path().join("json");

    // First run: convert
    crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--skip-validation")
        .arg("--skip-schema-validation")
        .assert()
        .success();

    // Create an orphaned .json file
    fs::write(output_dir.join("orphan.json"), "{}")?;
    assert!(output_dir.join("orphan.json").exists());

    // Run with --prune
    crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--skip-validation")
        .arg("--skip-schema-validation")
        .arg("--prune")
        .assert()
        .success()
        .stderr(predicate::str::contains("1 pruned"));

    // Orphan should be gone, original should remain
    assert!(!output_dir.join("orphan.json").exists());
    assert!(output_dir.join("a.json").exists());
    Ok(())
}

/// An input with no transcript (an empty directory, such as an unmounted
/// mount point) is refused before anything is converted or pruned: `--prune`
/// over it would read every JSON file under `--output-dir` as an orphan and
/// delete it.
#[test]
fn test_to_json_prune_refuses_an_empty_input() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_dir = dir.path().join("mount-point");
    fs::create_dir_all(&input_dir)?;
    let output_dir = dir.path().join("json");
    fs::create_dir_all(output_dir.join("corpus"))?;
    fs::write(output_dir.join("corpus/a.json"), "{}")?;

    crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .args(["--skip-validation", "--skip-schema-validation", "--prune"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no .cha files found"));

    assert!(
        output_dir.join("corpus/a.json").exists(),
        "an empty input pruned the output tree"
    );
    Ok(())
}

/// `--prune` never reaches through a link in the output tree: a linked
/// directory `json/shared -> elsewhere` keeps `elsewhere/x.json`, and the
/// empty-directory climb removes nothing outside `--output-dir`.
#[cfg(unix)]
#[test]
fn test_to_json_prune_does_not_follow_links_out_of_the_output_tree() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_dir = dir.path().join("corpus");
    fs::create_dir_all(&input_dir)?;
    fs::write(input_dir.join("a.cha"), VALID_CHAT)?;
    let output_dir = dir.path().join("json");
    fs::create_dir_all(&output_dir)?;
    let elsewhere = dir.path().join("elsewhere");
    fs::create_dir_all(elsewhere.join("nested"))?;
    fs::write(elsewhere.join("nested/x.json"), "{}")?;
    std::os::unix::fs::symlink(&elsewhere, output_dir.join("shared"))?;
    std::os::unix::fs::symlink(elsewhere.join("nested/x.json"), output_dir.join("y.json"))?;

    crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .args(["--skip-validation", "--skip-schema-validation", "--prune"])
        .assert()
        .success()
        .stderr(predicate::str::contains("0 pruned"));

    assert!(
        elsewhere.join("nested/x.json").exists(),
        "deleted through a link"
    );
    assert!(
        elsewhere.join("nested").is_dir(),
        "a directory outside was removed"
    );
    assert!(
        output_dir.join("shared").exists(),
        "the link itself was removed"
    );
    assert!(output_dir.join("a.json").exists());
    Ok(())
}

/// Tests directory mode requires --output-dir.
#[test]
fn test_to_json_directory_requires_output_dir() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input_dir = dir.path().join("corpus");
    fs::create_dir_all(&input_dir)?;
    fs::write(input_dir.join("a.cha"), VALID_CHAT)?;

    crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&input_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("--output-dir"));
    Ok(())
}

// ============================================================================
// FromJson Command Tests
// ============================================================================

/// Tests from json roundtrip.
#[test]
fn test_from_json_roundtrip() -> Result<(), TestError> {
    let dir = tempdir()?;
    let chat_path = dir.path().join("input.cha");
    let json_path = dir.path().join("intermediate.json");
    let output_path = dir.path().join("output.cha");

    fs::write(&chat_path, VALID_CHAT)?;

    // CHAT → JSON
    crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&chat_path)
        .arg("--output")
        .arg(&json_path)
        .assert()
        .success();

    // JSON → CHAT
    crate::common::chatter_cmd()
        .arg("from-json")
        .arg(&json_path)
        .arg("--output")
        .arg(&output_path)
        .assert()
        .success();

    // Verify output is valid CHAT
    if !output_path.exists() {
        return Err(TestError::Failure("Expected output file".to_string()));
    }
    let content = fs::read_to_string(&output_path)?;
    if !content.contains("@UTF8") || !content.contains("*CHI:") {
        return Err(TestError::Failure(
            "Output should contain CHAT headers".to_string(),
        ));
    }
    Ok(())
}

/// Tests from json invalid json.
#[test]
fn test_from_json_invalid_json() -> Result<(), TestError> {
    let json_file = NamedTempFile::new()?;
    fs::write(json_file.path(), "{ invalid json ")?;

    crate::common::chatter_cmd()
        .arg("from-json")
        .arg(json_file.path())
        .assert()
        .failure();
    Ok(())
}

// ============================================================================
// ShowAlignment Command Tests
// ============================================================================

/// Tests show alignment basic.
#[test]
fn test_show_alignment_basic() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("aligned.cha");
    fs::write(&file_path, VALID_CHAT)?;

    crate::common::chatter_cmd()
        .arg("show-alignment")
        .arg(&file_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Alignment"));
    Ok(())
}

/// Tests show alignment specific tier.
#[test]
fn test_show_alignment_specific_tier() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("aligned.cha");
    fs::write(&file_path, VALID_CHAT)?;

    crate::common::chatter_cmd()
        .arg("show-alignment")
        .arg(&file_path)
        .arg("--tier")
        .arg("mor")
        .assert()
        .success();
    Ok(())
}

/// Tests show alignment compact mode.
#[test]
fn test_show_alignment_compact_mode() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("aligned.cha");
    fs::write(&file_path, VALID_CHAT)?;

    let normal_output = crate::common::chatter_cmd()
        .arg("show-alignment")
        .arg(&file_path)
        .output()?;

    let compact_output = crate::common::chatter_cmd()
        .arg("show-alignment")
        .arg(&file_path)
        .arg("--compact")
        .output()?;

    // Compact output should be shorter
    if compact_output.stdout.len() > normal_output.stdout.len() {
        return Err(TestError::Failure(
            "Compact mode should produce less or equal output".to_string(),
        ));
    }
    Ok(())
}

// ============================================================================
// Help and Version Tests
// ============================================================================

/// Tests help command.
#[test]
fn test_help_command() {
    crate::common::chatter_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("validate"))
        .stdout(predicate::str::contains("normalize"))
        .stdout(predicate::str::contains("to-json"));
}

/// Tests validate help.
#[test]
fn test_validate_help() {
    crate::common::chatter_cmd()
        .arg("validate")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--skip-alignment"))
        .stdout(predicate::str::contains("--force"));
}

/// Tests no args shows help.
#[test]
fn test_no_args_shows_help() {
    crate::common::chatter_cmd()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

// ============================================================================
// Error Handling Tests
// ============================================================================

/// Tests error exit codes.
#[test]
fn test_error_exit_codes() -> Result<(), TestError> {
    let dir = tempdir()?;

    // Valid file: exit code 0
    let valid_path = dir.path().join("valid.cha");
    fs::write(&valid_path, VALID_CHAT)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&valid_path)
        .assert()
        .code(0);

    // Invalid file: exit code != 0
    let invalid_path = dir.path().join("invalid.cha");
    fs::write(&invalid_path, INVALID_CHAT_MISSING_END)?;

    let assert = crate::common::chatter_cmd()
        .arg("validate")
        .arg(&invalid_path)
        .assert()
        .failure();

    // Verify non-zero exit code
    assert.code(predicate::ne(0));
    Ok(())
}

/// Tests missing required argument.
#[test]
fn test_missing_required_argument() {
    crate::common::chatter_cmd()
        .arg("validate")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

// ============================================================================
// Exit Code Contract Tests
// ============================================================================

/// Exit code 0 for a valid CHAT file (CI contract).
#[test]
fn exit_code_zero_for_valid_file() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file = dir.path().join("valid.cha");
    fs::write(&file, VALID_CHAT)?;

    crate::common::chatter_cmd()
        .args(["validate", file.to_str().unwrap(), "--tui-mode", "disable"])
        .assert()
        .success(); // exit code 0
    Ok(())
}

/// Exit code 1 for an invalid CHAT file (CI contract).
#[test]
fn exit_code_nonzero_for_invalid_file() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file = dir.path().join("invalid.cha");
    fs::write(&file, INVALID_CHAT_MISSING_END)?;

    crate::common::chatter_cmd()
        .args(["validate", file.to_str().unwrap(), "--tui-mode", "disable"])
        .assert()
        .failure(); // exit code != 0
    Ok(())
}

/// Exit code 1 for a nonexistent file path (CI contract).
#[test]
fn exit_code_nonzero_for_nonexistent_file() {
    crate::common::chatter_cmd()
        .args([
            "validate",
            "/tmp/nonexistent_file_12345.cha",
            "--tui-mode",
            "disable",
        ])
        .assert()
        .failure();
}

/// Exit code 2 for missing required arguments (clap usage error).
#[test]
fn exit_code_two_for_usage_error() {
    crate::common::chatter_cmd()
        .args(["validate"])
        .assert()
        .code(2);
}

/// Tests that --help includes a Getting Started section for new users.
#[test]
fn help_text_includes_getting_started() -> Result<(), TestError> {
    crate::common::chatter_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Getting started"));
    Ok(())
}

// ============================================================================
// Strict Linkers (--strict-linkers) Tests
// ============================================================================

/// CHAT file with self-completion linker (+,) and no preceding interrupted
/// utterance. Without --strict-linkers this should pass; with it, E351 fires.
const CHAT_WITH_SELF_COMPLETION_ORPHAN: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\t+, hello world .\n@End\n";

/// Every flag a spec example tells a reader to pass is a flag this binary has.
///
/// SURVIVES a type change, and says which category: this reaches the OUTSIDE
/// world, namely a subprocess and its argument parser. `RuleProfile::cli_flag`
/// lives in the spec workspace and clap's `long` lives here; no signature
/// relates them, and the spec workspace cannot see clap at all.
///
/// What it buys: a generated error page for an opt-in code says "requires
/// `--strict-linkers`", and a reader copies that into a terminal. Rename the
/// clap `long` and, without this, every gate stays green while eight
/// published pages advertise a flag the binary rejects. The doc on `cli_flag`
/// claimed this test existed before it did, which is the same defect one
/// level up.
#[test]
fn every_rules_profile_flag_is_a_flag_this_binary_accepts() -> Result<(), TestError> {
    use talkbank_spec_vocabulary::frontmatter::RuleProfile;

    let dir = tempdir()?;
    let file_path = dir.path().join("valid.cha");
    fs::write(&file_path, VALID_CHAT)?;

    for profile in RuleProfile::ALL {
        let Some(flag) = profile.cli_flag() else {
            continue;
        };
        // Passing it must be ACCEPTED, not merely present in the help text: a
        // flag can be documented and removed, and a `--help` grep would still
        // find it in a sentence describing something else.
        crate::common::chatter_cmd()
            .arg("validate")
            .arg(flag)
            .arg(&file_path)
            .assert()
            .success();
    }
    Ok(())
}

/// Self-completion orphan passes validation without --strict-linkers.
#[test]
fn strict_linkers_off_allows_orphan_self_completion() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("orphan.cha");
    fs::write(&file_path, CHAT_WITH_SELF_COMPLETION_ORPHAN)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .assert()
        .success();
    Ok(())
}

// ============================================================================
// Cascading Error Hint Tests
// ============================================================================

/// When structural errors (E1xx-E5xx) are present but no alignment errors
/// (E7xx) were emitted, the validator should hint that alignment checks
/// may not have run because of the structural errors.
#[test]
fn cascading_error_hint_shown_for_structural_errors() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("missing_end.cha");
    fs::write(&file_path, INVALID_CHAT_MISSING_END)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "additional checks may not have run",
        ));
    Ok(())
}

/// A valid file should not show any cascading error hint.
#[test]
fn no_cascading_hint_for_valid_file() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("valid.cha");
    fs::write(&file_path, VALID_CHAT)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("additional checks may not have run").not())
        .stderr(predicate::str::contains("additional checks may not have run").not());
    Ok(())
}

// ============================================================================
// Strict Linkers (--strict-linkers) Tests
// ============================================================================

/// Self-completion orphan triggers E351 when --strict-linkers is enabled.
#[test]
fn strict_linkers_on_rejects_orphan_self_completion() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("orphan.cha");
    fs::write(&file_path, CHAT_WITH_SELF_COMPLETION_ORPHAN)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg("--strict-linkers")
        .arg(&file_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("E351"));
    Ok(())
}

// ============================================================================
// E302: the code no spec fixture can carry
// ============================================================================

/// A main tier with a replacement whose text is empty, and NO trailing newline.
///
/// The missing final newline is the whole point and is why this constant is
/// here rather than in `spec/errors/E302.md`: the fixture generator appends
/// exactly one newline to every fixture it writes, deliberately, so that a
/// fixture is never testing its rule plus a MISSING-newline recovery node. No
/// spec example can express this input.
const REPLACEMENT_AT_EOF_WITHOUT_NEWLINE: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello [: ] .";

/// Empty replacements remain invalid with or without the final newline.
///
/// This is a file-byte boundary the fixture generator normalizes away. The
/// source-bound whole-file traversal now retains the replacement's structural
/// diagnostic in both cases; do not reintroduce the old fragment reparse merely
/// to preserve its E302 diagnostic identity.
#[test]
fn empty_replacement_at_eof_is_rejected_with_or_without_newline() -> Result<(), TestError> {
    let dir = tempdir()?;

    let at_eof = dir.path().join("no_final_newline.cha");
    fs::write(&at_eof, REPLACEMENT_AT_EOF_WITHOUT_NEWLINE)?;
    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&at_eof)
        .assert()
        .failure()
        .stderr(predicate::str::contains("E376"));

    let with_newline = dir.path().join("with_final_newline.cha");
    fs::write(
        &with_newline,
        format!("{REPLACEMENT_AT_EOF_WITHOUT_NEWLINE}\n"),
    )?;
    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&with_newline)
        .assert()
        .failure()
        .stderr(predicate::str::contains("E376"));

    Ok(())
}

// ============================================================================
// CLAN Help Grouping
// ============================================================================

// ============================================================================
// --list-checks
// ============================================================================

/// `chatter validate --list-checks` prints every check and exits 0 without
/// requiring a path argument. Output must mention both statuses so users know
/// the column exists.
#[test]
fn list_checks_shows_active_and_planned() -> Result<(), TestError> {
    crate::common::chatter_cmd()
        .args(["validate", "--list-checks", "--tui-mode", "disable"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Active"))
        .stdout(predicate::str::contains("Planned"))
        // Spot-check a known Active and a known Planned code.
        .stdout(predicate::str::contains("E305"))
        .stdout(predicate::str::contains("E321"));
    Ok(())
}

// ============================================================================
// Debug Sanitize Tests (`chatter debug sanitize`)
// ============================================================================

/// Source words must not appear in sanitize output written to stdout.
#[test]
fn test_sanitize_stdout_strips_words() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input = dir.path().join("input.cha");
    fs::write(&input, VALID_CHAT)?;

    crate::common::chatter_cmd()
        .arg("debug")
        .arg("sanitize")
        .arg(&input)
        .assert()
        .success()
        .stdout(predicate::str::contains("hello").not())
        .stdout(predicate::str::contains("world").not())
        .stdout(predicate::str::contains("*CHI:"))
        .stdout(predicate::str::contains("w1"))
        .stdout(predicate::str::contains("w2"));
    Ok(())
}

/// `--output` writes the sanitized result to a file.
#[test]
fn test_sanitize_output_file() -> Result<(), TestError> {
    let dir = tempdir()?;
    let input = dir.path().join("input.cha");
    let output = dir.path().join("sanitized.cha");
    fs::write(&input, VALID_CHAT)?;

    crate::common::chatter_cmd()
        .arg("debug")
        .arg("sanitize")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .assert()
        .success();

    let body = fs::read_to_string(&output)?;
    assert!(
        !body.contains("hello"),
        "source word leaked into file:\n{body}"
    );
    assert!(
        !body.contains("world"),
        "source word leaked into file:\n{body}"
    );
    assert!(body.contains("*CHI:"), "speaker code missing:\n{body}");
    assert!(body.contains("w1"), "placeholder w1 missing:\n{body}");
    Ok(())
}

/// A `@u` phonetic form (UNIBET/IPA in a word slot) must validate clean
/// and must surface in `to-json` as TYPED PHONETIC content, not as an
/// orthographic text node. The fixture is the real aphasia pattern:
/// phonetic surface + `[: target]` replacement + `[* code]` error.
#[test]
fn test_to_json_types_u_form_content_as_phonetic() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("unibet.cha");
    fs::write(
        &file_path,
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
         @ID:\teng|test|CHI|3;|female|||Target_Child|||\n\
         *CHI:\ts\u{026a}nd\u{259}\u{02de}w\u{0251}n@u [: syndrome] [* n:k] .\n@End\n",
    )?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .assert()
        .success();

    let output = crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&file_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let doc: serde_json::Value = serde_json::from_slice(&output)
        .map_err(|e| TestError::Failure(format!("to-json emitted invalid JSON: {e}")))?;

    // Find the @u word node anywhere in the document. Word objects are
    // not always tagged with "type":"word" (e.g. inside replaced_word),
    // so the signature is form_type:"u" plus a content array.
    fn find_u_word(node: &serde_json::Value) -> Option<&serde_json::Value> {
        match node {
            serde_json::Value::Object(map) => {
                if map.get("form_type").and_then(|t| t.as_str()) == Some("u")
                    && map.get("content").is_some_and(|c| c.is_array())
                {
                    return Some(node);
                }
                map.values().find_map(find_u_word)
            }
            serde_json::Value::Array(items) => items.iter().find_map(find_u_word),
            _ => None,
        }
    }
    let word = find_u_word(&doc)
        .ok_or_else(|| TestError::Failure("no @u word in to-json output".to_string()))?;

    // The provenance requirement: the content is a typed phonetic node,
    // never an orthographic "text" node (the 2026-07-13 UNIBET design,
    // option B; the maintainer's 2026-07-14 ruling: @u only).
    let content = word["content"]
        .as_array()
        .ok_or_else(|| TestError::Failure("@u word has no content array".to_string()))?;
    let kinds: Vec<&str> = content.iter().filter_map(|c| c["type"].as_str()).collect();
    if kinds.contains(&"text") {
        return Err(TestError::Failure(format!(
            "@u word content is orthographic text, expected typed phonetic: {kinds:?}"
        )));
    }
    if !kinds.contains(&"phonetic") {
        return Err(TestError::Failure(format!(
            "@u word content lacks a phonetic node: {kinds:?}"
        )));
    }
    Ok(())
}

/// A mid-word syllable pause chain (`or^ga^ni^zi^ra`, `rhi^noceros`) is
/// ONE word and validates clean. Regression: the v0.3.3 overlap-port
/// grammar (6192c178) weighted interior overlap readings with
/// prec.dynamic(2) but left `syllable_pause` out of the interior lead
/// set, so GLR tie-breaking fragmented `or^ga^ni^zi^ra` into `or` +
/// `^ga^ni^zi^ra`, and E252 then fired on the caret-initial fragment
/// (2026-07-15 field report from real Croatian aphasia-protocol data).
#[test]
fn test_mid_word_syllable_pause_chain_is_one_word_and_valid() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("sylpause.cha");
    fs::write(
        &file_path,
        "@UTF8\n@Begin\n@Languages:\thrv\n@Participants:\tPAR Participant\n\
         @ID:\thrv|test|PAR|||||Participant|||\n\
         *PAR:\tneka or^ga^ni^zi^ra ba [///] \u{0161}ta je .\n\
         *PAR:\trhi^noceros je .\n@End\n",
    )?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .assert()
        .success();

    let output = crate::common::chatter_cmd()
        .arg("to-json")
        .arg(&file_path)
        .arg("--skip-validation")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let doc: serde_json::Value = serde_json::from_slice(&output)
        .map_err(|e| TestError::Failure(format!("to-json emitted invalid JSON: {e}")))?;

    // The multi-caret token must surface as ONE word whose raw_text is the
    // full chain: fragmentation (a word `or` plus a caret-initial word)
    // is exactly the regression this test pins.
    fn collect_words(node: &serde_json::Value, out: &mut Vec<String>) {
        match node {
            serde_json::Value::Object(map) => {
                if map.get("type").and_then(|t| t.as_str()) == Some("word")
                    && let Some(raw) = map.get("raw_text").and_then(|r| r.as_str())
                {
                    out.push(raw.to_string());
                }
                map.values().for_each(|v| collect_words(v, out));
            }
            serde_json::Value::Array(items) => items.iter().for_each(|v| collect_words(v, out)),
            _ => {}
        }
    }
    let mut words = Vec::new();
    collect_words(&doc, &mut words);
    if !words.iter().any(|w| w == "or^ga^ni^zi^ra") {
        return Err(TestError::Failure(format!(
            "or^ga^ni^zi^ra did not survive as one word; words seen: {words:?}"
        )));
    }
    if !words.iter().any(|w| w == "rhi^noceros") {
        return Err(TestError::Failure(format!(
            "rhi^noceros did not survive as one word; words seen: {words:?}"
        )));
    }
    Ok(())
}
