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

//! Execution-coverage tests for shipping top-level subcommands.
//!
//! `command_surface_manifest.rs` proves every published subcommand is
//! listed in `--help`, but that is only a help-contract check: several
//! shipping subcommands had no test that actually RAN them against real
//! input. The gap (confirmed 2026-06-13) covered `clean`,
//! `lint`, `new-file`, `schema`, `validate-utseg`, and `watch`.
//!
//! This file closes the gap with subprocess-level characterization tests
//! that pin each command's current, known-good behavior. They run the real
//! CLI seam (the boundary a user hits), use reference-corpus fixtures
//! (never ad hoc CHAT, per the test-file policy), and isolate the
//! validation cache through `CliHarness` (mandated for every CLI
//! integration test). A red here is a real regression in a command we
//! ship, not a flaky expectation.

#[cfg(not(unix))]
use std::fs;
use std::path::Path;

use predicates::prelude::*;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

use crate::common::{CliHarness, assert_success, parse_json, reference_fixture};

/// Content-rich reference fixture: two speakers, several utterances with
/// real words.
const CONVERSATION_FIXTURE: &str = "corpus/reference/core/basic-conversation.cha";

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
// to-xml: REMOVED 2026-07-27
// ============================================================================
//
// `chatter to-xml` and the whole XML surface were removed. TalkBank stopped
// generating TalkBank XML on 2025-10-29, when the last consumer said he no
// longer used it. Three `to_xml_*` tests lived here; the
// removal is guarded instead by `help_offers_no_xml_command` in
// `cli/args/core_tests.rs`, which asserts the CLI offers no XML command in
// either direction.

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
// lint (removed; the span-splicing `fix` command is the intended successor)
// ============================================================================

/// `chatter lint` was deleted (unmaintained span-driven byte writer with no
/// dummy-span, char-boundary, or overlap protection; zero production
/// callers). Clap must reject the unknown subcommand rather than silently
/// accepting it.
#[test]
fn lint_subcommand_is_gone() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    harness
        .chatter_cmd()
        .args(["lint", "--help"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
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

/// `--max-errors 0` is a usage error (exit 2): a limit of zero errors is not
/// a limit anything can reach. The count is parsed as a `NonZeroUsize` at
/// the argument boundary, as `--jobs` is.
#[test]
fn max_errors_zero_is_a_usage_error() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    harness
        .chatter_cmd()
        .arg("validate")
        .arg(dir.path())
        .args(["--max-errors", "0"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--max-errors"));
    Ok(())
}

/// `--jobs 0` is a usage error (exit 2) on every command that takes it, not a
/// request the runner quietly turns into one worker. The count is parsed as a
/// `NonZeroUsize` at the argument boundary, so no zero reaches the runner.
#[test]
fn jobs_zero_is_a_usage_error() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let out_dir = dir.path().join("json");
    let validate_long = harness
        .chatter_cmd()
        .arg("validate")
        .arg(dir.path())
        .args(["--jobs", "0"])
        .assert();
    let validate_short = harness
        .chatter_cmd()
        .arg("validate")
        .arg(dir.path())
        .args(["-j", "0"])
        .assert();
    let to_json = harness
        .chatter_cmd()
        .arg("to-json")
        .arg(dir.path())
        .arg("--output-dir")
        .arg(&out_dir)
        .args(["--jobs", "0"])
        .assert();
    for refused in [validate_long, validate_short, to_json] {
        refused.code(2).stderr(predicate::str::contains("--jobs"));
    }
    Ok(())
}

// ============================================================================
// program name (cross-platform identity)
// ============================================================================

/// The program name shown in usage/help/error output must be the pinned
/// brand name `chatter`, identical on every platform and independent of
/// how the binary was invoked.
///
/// clap otherwise derives the displayed name from `argv[0]`'s file name.
/// On Windows the binary is `chatter.exe`, so every usage line read
/// `Usage: chatter.exe ...`, diverging from the `chatter ...` invocation
/// documented throughout the book and breaking the help-contract tests
/// (`watch_help_documents_command` on windows-latest, cross-platform CI).
///
/// This reproduces the mechanism by running the real binary under a
/// deliberately different `argv[0]`: set it directly on Unix, and invoke a
/// renamed hard link on Windows. Without a pinned `bin_name` the usage line leaks the
/// renamed file; with it pinned the line is always `Usage: chatter ...`.
/// It would have caught the Windows regression on Ubuntu CI.
#[test]
fn program_name_is_pinned_regardless_of_argv0() -> Result<(), TestError> {
    let bin = Path::new(env!("CARGO_BIN_EXE_chatter"));
    // Unix exposes argv[0] directly. Keep the executable's filesystem identity
    // untouched while other tests launch it: macOS security logs have reported
    // denials for the hard-linked alias during concurrent CLI test runs.
    #[cfg(unix)]
    let executable = bin;

    // Windows has no CommandExt::arg0. A same-filesystem hard link changes the
    // name without introducing an open-for-write executable (ETXTBSY on Unix).
    #[cfg(not(unix))]
    let scratch = tempfile::Builder::new()
        .prefix("argv0-probe-")
        .tempdir_in(bin.parent().expect("test binary has a parent directory"))?;
    #[cfg(not(unix))]
    let executable = scratch
        .path()
        .join(format!("renamed-probe{}", std::env::consts::EXE_SUFFIX));
    #[cfg(not(unix))]
    fs::hard_link(bin, &executable)?;

    // The top-level command and a subcommand both build their usage line
    // from the program name; pin must hold for both.
    for args in [["--help"].as_slice(), ["watch", "--help"].as_slice()] {
        let mut command = std::process::Command::new(executable.as_os_str());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.arg0("renamed-probe");
        }
        let output = command.args(args).output()?;
        assert_success(&output, "renamed argv[0] help");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Usage: chatter"),
            "usage line must use the pinned program name `chatter`, not the \
             executable file name; args={args:?}\nstdout:\n{stdout}"
        );
        assert!(
            !stdout.contains("renamed-probe"),
            "usage line leaked the executable file name instead of the pinned \
             program name; args={args:?}\nstdout:\n{stdout}"
        );
    }
    Ok(())
}

/// Output flags that name two surfaces are usage errors (exit 2), never a
/// silent winner: `--audit` beside `--format json`, and `--format json
/// --quiet`.
#[test]
fn conflicting_output_flags_are_usage_errors() -> Result<(), TestError> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("a.cha");
    std::fs::write(
        &file,
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello .\n@End\n",
    )?;
    let audit = dir.path().join("out.jsonl");
    for args in [
        vec![
            "--audit",
            audit.to_str().expect("utf-8"),
            "--format",
            "json",
        ],
        vec!["--audit", audit.to_str().expect("utf-8"), "--quiet"],
        vec!["--format", "json", "--quiet"],
        vec!["--format", "json", "--tui-mode", "force"],
        // An audit never writes the cache; --force would clear rows.
        vec!["--audit", audit.to_str().expect("utf-8"), "--force"],
    ] {
        let output = crate::common::chatter_cmd()
            .arg("validate")
            .arg(&file)
            .args(&args)
            .output()?;
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(!audit.exists(), "{args:?} wrote the audit file");
    }
    Ok(())
}

/// A flag that a command would ignore is a usage error instead:
/// `normalize --skip-alignment` without `--validate`, and `to-json
/// --skip-validation --skip-alignment` (skipping validation already skips
/// alignment).
#[test]
fn flags_a_command_would_ignore_are_usage_errors() -> Result<(), TestError> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("a.cha");
    std::fs::write(
        &file,
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello .\n@End\n",
    )?;
    for args in [
        vec!["normalize", "--skip-alignment"],
        vec!["to-json", "--skip-validation", "--skip-alignment"],
    ] {
        let output = crate::common::chatter_cmd()
            .args(&args[..1])
            .arg(&file)
            .args(&args[1..])
            .output()?;
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
    }
    Ok(())
}
