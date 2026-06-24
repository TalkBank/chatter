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

mod common;

use common::{CliHarness, combined_output, write_fixture};

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
