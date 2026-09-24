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

//! CLI integration coverage for `@Media` filename matching (E531, CLAN CHECK
//! 157).
//!
//! These exercise the real `chatter validate` boundary, the validation_runner
//! worker, because that path is where the bug lived: it passed `None` for the
//! datafile name, so `check_media_filename_match` never ran from the CLI even
//! though it ran in the in-process parity harness (which passes the stem). A
//! manifest entry alone would have masked the gap; only a subprocess test pins
//! the plumbing.

use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

use crate::common::{CliHarness, combined_output, write_fixture};

const MEDIA_NFD: &str =
    include_str!("../../../talkbank-parser-tests/tests/error_corpus/validation_errors/W109_2.cha");
const MEDIA_NFC: &str =
    include_str!("../../../talkbank-parser-tests/tests/error_corpus/validation_errors/W109_4.cha");

/// The real fixer changes only the canonical spec's media token, never the
/// directory entry. A file-side warning may survive a successful content fix.
#[test]
fn w109_fix_preserves_stored_name_and_unrelated_bytes() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    for stored_name in ["Schlüssel.cha", "Schlu\u{308}ssel.cha"] {
        let dir = tempdir()?;
        let path = write_fixture(dir.path(), stored_name, MEDIA_NFD)?;
        let result = harness
            .chatter_cmd()
            .args(["fix", "--code", "W109", "--apply"])
            .arg(&path)
            .output()?;
        let text = combined_output(&result);
        assert!(result.status.success(), "{text}");
        assert!(text.contains("1 fix(es) applied"), "{text}");
        assert_eq!(std::fs::read_to_string(&path)?, MEDIA_NFC);
        let names: Vec<_> = std::fs::read_dir(dir.path())?
            .map(|entry| entry.map(|e| e.file_name()))
            .collect::<Result<_, _>>()?;
        assert_eq!(names, [std::ffi::OsString::from(stored_name)]);
        let after = combined_output(&harness.run_validate(&path, &["--force"])?);
        assert!(!after.contains("E531"), "{after}");
        assert!(!after.contains("The @Media name uses"), "{after}");
        assert_eq!(
            after.contains("W109"),
            stored_name.contains('\u{308}'),
            "{after}"
        );
        let again = harness
            .chatter_cmd()
            .args(["fix", "--code", "W109", "--apply"])
            .arg(&path)
            .output()?;
        assert!(combined_output(&again).contains("0 fix(es) applied"));
        assert_eq!(std::fs::read_to_string(&path)?, MEDIA_NFC);
    }
    Ok(())
}

/// APFS lookup aliases must not change which stored-name side needs repair.
#[cfg(target_os = "macos")]
#[test]
fn w109_uses_stored_name_for_nfc_nfd_and_directory_arguments() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let nfd = write_fixture(dir.path(), "Schlu\u{308}ssel.cha", MEDIA_NFD)?;
    let nfc = dir.path().join("Schlüssel.cha");
    // A normalization-sensitive volume does not offer the alias under test.
    if !nfc.exists() {
        return Ok(());
    }
    for input in [&nfc, &nfd, &dir.path().to_path_buf()] {
        let text = combined_output(&harness.run_validate(input, &["--force"])?);
        assert!(text.contains("W109"), "{text}");
        assert!(text.contains("The @Media name uses"), "{text}");
        assert!(text.contains("The file name"), "{text}");
        assert!(!text.contains("E531"), "{text}");
    }
    Ok(())
}

/// Remote URL spelling is opaque: W109 never rewrites URL code points.
#[test]
fn w109_fix_does_not_normalize_media_urls() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir()?;
    let source = fixture_with_media("\"https://example.org/Schlu\u{308}ssel.mp3\"");
    let path = write_fixture(dir.path(), "session.cha", &source)?;
    let result = harness
        .chatter_cmd()
        .args(["fix", "--code", "W109", "--apply"])
        .arg(&path)
        .output()?;
    assert!(result.status.success());
    assert!(combined_output(&result).contains("0 fix(es) applied"));
    assert_eq!(std::fs::read_to_string(path)?, source);
    Ok(())
}

/// A minimal valid preamble plus a `@Media` line and one timing bullet (so the
/// linkage checks E544/E552 do not also fire and the only `@Media`-related
/// signal is the filename match). `{media}` is the `@Media` first field.
fn fixture_with_media(media: &str) -> String {
    format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
         @ID:\teng|corpus|CHI|||||Target_Child|||\n@Media:\t{media}, audio\n\
         *CHI:\thello .\u{15}0_1500\u{15}\n@End\n"
    )
}

/// A `@Media` filename that differs from the datafile basename is rejected
/// (E531) when validated through the real CLI.
#[test]
fn media_filename_mismatch_is_rejected_via_cli() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir().map_err(|e| TestError::Failure(format!("tempdir: {e}")))?;
    // Basename is `session`; the @Media names a different file.
    let path = write_fixture(
        dir.path(),
        "session.cha",
        &fixture_with_media("differentname"),
    )?;
    let output = harness.run_validate(&path, &["--force"])?;
    let text = combined_output(&output);
    assert!(
        text.contains("E531"),
        "expected E531 for an @Media filename that does not match the datafile basename, got:\n{text}"
    );
    Ok(())
}

/// A remote URL `@Media` is exempt from the filename-match rule (CLAN itself
/// accepts `@Media: "https://..."` with no CHECK 157).
#[test]
fn media_url_is_exempt_from_filename_match() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir().map_err(|e| TestError::Failure(format!("tempdir: {e}")))?;
    let path = write_fixture(
        dir.path(),
        "session.cha",
        &fixture_with_media("\"https://media.talkbank.org/x.mp3\""),
    )?;
    let output = harness.run_validate(&path, &["--force"])?;
    let text = combined_output(&output);
    assert!(
        !text.contains("E531"),
        "a URL @Media must be exempt from the filename-match rule (E531), got:\n{text}"
    );
    Ok(())
}

/// A `@Media` filename equal to the datafile basename does not emit E531.
#[test]
fn matching_media_filename_is_accepted() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir().map_err(|e| TestError::Failure(format!("tempdir: {e}")))?;
    let path = write_fixture(dir.path(), "session.cha", &fixture_with_media("session"))?;
    let output = harness.run_validate(&path, &["--force"])?;
    let text = combined_output(&output);
    assert!(
        !text.contains("E531"),
        "a matching @Media filename must not emit E531, got:\n{text}"
    );
    Ok(())
}

/// E531 also runs on the `to-json` path, not only on `validate`.
///
/// # Why this exists
///
/// `chat_to_json` took content and no name, so `parse_and_validate` passed
/// `None` for the filename and every rule about the transcript's own name was
/// silently skipped. That was known and DOCUMENTED rather than fixed: the
/// pipeline carried a `NOTE` plus a `FOLLOW-UP` in production source saying
/// E531 does not run for `to-json` or any other pipeline consumer. A hazard
/// note is a bug with documentation, and this is the test that closes it.
///
/// The sibling assertion below matters as much: turning a rule ON is only
/// correct if it still stays quiet when it should. Without it this test would
/// pass equally well against code that reported E531 on every file.
#[test]
fn media_filename_mismatch_is_rejected_via_to_json() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir().map_err(|e| TestError::Failure(format!("tempdir: {e}")))?;
    let path = write_fixture(dir.path(), "session.cha", &fixture_with_media("elsewhere"))?;

    for skip_schema in [false, true] {
        let mut cmd = harness.chatter_cmd();
        cmd.arg("to-json").arg(&path);
        if skip_schema {
            cmd.arg("--skip-schema-validation");
        }
        let output = cmd.output()?;
        let text = combined_output(&output);
        assert!(
            !output.status.success(),
            "mismatched media must fail: {text}"
        );
        assert!(
            text.contains("E531"),
            "schema policy must preserve E531: {text}"
        );
    }
    Ok(())
}

/// ...and stays quiet on `to-json` when the names agree.
#[test]
fn matching_media_filename_is_accepted_via_to_json() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir().map_err(|e| TestError::Failure(format!("tempdir: {e}")))?;
    let path = write_fixture(dir.path(), "session.cha", &fixture_with_media("session"))?;

    for skip_schema in [false, true] {
        let mut cmd = harness.chatter_cmd();
        cmd.arg("to-json").arg(&path);
        if skip_schema {
            cmd.arg("--skip-schema-validation");
        }
        let output = cmd.output()?;
        let text = combined_output(&output);
        assert!(
            output.status.success(),
            "matching media must convert: {text}"
        );
        assert!(
            !text.contains("E531"),
            "matching media must stay valid: {text}"
        );
    }
    Ok(())
}

/// An `@Media` name that is the same name as the datafile basename under
/// Unicode canonical equivalence, but spelled with a different normalization
/// form (NFD: base letter + combining mark, vs. NFC: the precomposed
/// codepoint), is not a real filename mismatch. Transcripts can carry both
/// spellings for the same recording; this must
/// not report E531.
#[test]
fn nfd_media_name_matching_an_nfc_datafile_name_is_not_a_mismatch() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir().map_err(|e| TestError::Failure(format!("tempdir: {e}")))?;
    // "Schlüssel3": NFC in the filename (precomposed U+00FC), NFD in the
    // @Media name ("u" U+0075 + combining diaeresis U+0308).
    let nfc_name = "Schl\u{fc}ssel3";
    let nfd_name = "Schlu\u{308}ssel3";
    let path = write_fixture(
        dir.path(),
        &format!("{nfc_name}.cha"),
        &fixture_with_media(nfd_name),
    )?;
    let output = harness.run_validate(&path, &["--force"])?;
    let text = combined_output(&output);
    assert!(
        !text.contains("E531"),
        "NFC/NFD spellings of the same name must not be a filename mismatch, got:\n{text}"
    );
    assert!(
        text.contains("W109"),
        "NFC/NFD spellings of the same name must be flagged for canonicalization, got:\n{text}"
    );
    Ok(())
}

/// An `@Media` name and datafile basename that are both already NFC (the
/// common case) get neither E531 nor W109.
#[test]
fn matching_nfc_media_filename_has_no_canonicalization_warning() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir().map_err(|e| TestError::Failure(format!("tempdir: {e}")))?;
    let nfc_name = "Schl\u{fc}ssel3";
    let path = write_fixture(
        dir.path(),
        &format!("{nfc_name}.cha"),
        &fixture_with_media(nfc_name),
    )?;
    let output = harness.run_validate(&path, &["--force"])?;
    let text = combined_output(&output);
    assert!(!text.contains("E531"), "identical NFC names: {text}");
    assert!(
        !text.contains("W109"),
        "identical NFC names must not get a canonicalization warning: {text}"
    );
    Ok(())
}

/// Directory conversion must preserve each source filename under both schema policies.
#[test]
fn directory_to_json_preserves_filename_checks_when_schema_is_skipped() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir().map_err(|e| TestError::Failure(format!("tempdir: {e}")))?;
    let input = dir.path().join("input");
    std::fs::create_dir(&input)?;
    write_fixture(&input, "session.cha", &fixture_with_media("elsewhere"))?;
    write_fixture(&input, "matching.cha", &fixture_with_media("matching"))?;
    for skip_schema in [false, true] {
        let output = dir.path().join(format!("output-{skip_schema}"));
        let mut cmd = harness.chatter_cmd();
        cmd.arg("to-json")
            .arg(&input)
            .arg("--output-dir")
            .arg(&output);
        if skip_schema {
            cmd.arg("--skip-schema-validation");
        }
        let result = cmd.output()?;
        let text = combined_output(&result);
        assert!(
            !result.status.success(),
            "partial conversion must fail: {text}"
        );
        assert!(
            text.contains("E531"),
            "directory conversion must report E531: {text}"
        );
        assert!(
            !output.join("session.json").exists(),
            "invalid transcript was written"
        );
        assert!(
            output.join("matching.json").is_file(),
            "valid transcript was lost: {text}"
        );
    }
    Ok(())
}
