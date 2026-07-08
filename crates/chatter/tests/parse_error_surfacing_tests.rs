// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
)]

//! CLI integration coverage for surfacing tree-sitter recovery nodes as
//! invalidity (CLAN CHECK parity, the "recovery != validity" rule).
//!
//! The default tree-sitter parser recovers from malformed CHAT by inserting
//! `ERROR` / `MISSING` CST nodes and continuing, which is correct for the LSP
//! and downstream repair. But a recovered document is NOT valid: it did not
//! conform to the grammar. Historically chatter's lowering scanned only some
//! CST regions for recovery nodes, so several malformed files that CLAN CHECK
//! (and chatter's own re2c oracle) reject were silently accepted at the CLI.
//!
//! These exercise the real `chatter validate` boundary (the validation_runner
//! worker), because the bug lived in the parse-to-model lowering that the CLI
//! drives; an in-process parser test alone would not prove the plumbing, the
//! same lesson as the E531/CHECK-157 gap.
//!
//! - ERROR nodes surface as E316 (UnparsableContent).
//! - MISSING nodes surface as E342 (MissingRequiredElement).

use std::path::PathBuf;

use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

mod common;

use common::{CliHarness, combined_output, reference_fixture, write_fixture};

/// One committed CHECK-parity fixture (`crates/talkbank-parser-tests/...`).
fn parity_fixture(name: &str) -> PathBuf {
    reference_fixture(&format!(
        "crates/talkbank-parser-tests/tests/check_parity/fixtures/{name}"
    ))
}

/// Assert `chatter validate` rejects the committed CHECK-parity `fixture` with
/// `code`, through the real CLI (the validation_runner worker, the seam the
/// silent-accept bug hid behind).
fn assert_parity_fixture_rejected(fixture: &str, code: &str) -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let path = parity_fixture(fixture);
    let output = harness.run_validate(&path, &["--force"])?;
    let text = combined_output(&output);
    assert!(
        text.contains(code),
        "expected {code} (surviving recovery node) in `{fixture}`, got:\n{text}"
    );
    Ok(())
}

/// `@Begin:` (a colon illegal on the no-colon `@Begin` header) leaves a stray
/// `:` ERROR node that was silently dropped. CLAN CHECK 5/6.
#[test]
fn begin_with_illegal_colon_is_rejected_via_cli() -> Result<(), TestError> {
    assert_parity_fixture_rejected("CHECK_005_begin_illegal_colon.cha", "E316")
}

/// `@Begins` (a misspelled `@Begin`) leaves a trailing `s` ERROR node. CLAN
/// CHECK 6 (and incidentally 17).
#[test]
fn malformed_begin_header_is_rejected_via_cli() -> Result<(), TestError> {
    assert_parity_fixture_rejected("CHECK_006_begin_malformed.cha", "E316")
}

/// A scoped code split across a newline (`[% comment` then a continuation line
/// `continued]`) leaves an ERROR node in the main-tier content. CLAN CHECK 106.
#[test]
fn code_spanning_a_newline_is_rejected_via_cli() -> Result<(), TestError> {
    assert_parity_fixture_rejected("CHECK_106_code_spans_newline.cha", "E316")
}

/// A postcode (`[+ trn]`) placed after the final time bullet leaves an ERROR
/// node in the utterance. CLAN CHECK 108.
#[test]
fn postcode_after_final_bullet_is_rejected_via_cli() -> Result<(), TestError> {
    assert_parity_fixture_rejected("CHECK_108_postcode_after_bullet.cha", "E316")
}

/// A missing `@End` makes tree-sitter wrap the WHOLE document in one ERROR node
/// around the recovered headers/lines. The validation layer already reports that
/// precisely (E502, "Missing @End header"); the recovery-node backstop must NOT
/// also emit a misleading whole-file E316 over `@UTF8`. So a missing-`@End` file
/// must surface E502 and NOT E316. (Regression: the backstop reported the
/// structural wrapper, double-flagging the file and breaking audit-record counts.)
#[test]
fn missing_end_reports_e502_not_a_whole_file_e316() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir().map_err(|e| TestError::Failure(format!("tempdir: {e}")))?;
    // A valid preamble + utterance, but no `@End` line.
    let content = "@UTF8\n@Begin\n@Languages:\teng\n\
        @Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n\
        *CHI:\thello world .\n";
    let path = write_fixture(dir.path(), "session.cha", content)?;
    let output = harness.run_validate(&path, &["--force"])?;
    let text = combined_output(&output);
    assert!(
        text.contains("E502"),
        "expected E502 (missing @End) for a file with no @End, got:\n{text}"
    );
    assert!(
        !text.contains("E316"),
        "the recovery-node backstop must not report the whole-document ERROR wrapper as E316; \
         the validation layer owns the missing-@End diagnostic (E502), got:\n{text}"
    );
    Ok(())
}

/// A `<...>` group on the main tier with no following annotation. Both parsers
/// only parse it via a synthetic MISSING recovery (`retrace_complete`), but CLAN
/// rejects it ("< > should be followed by [ ]") and recovery is not validity.
/// The content is built inline rather than read from `corpus/reference/`, which
/// holds only valid CHAT. (No CLAN `(NN)` code, so it is grounded by spec/errors
/// + these tests, not the CHECK-parity manifest.)
const GROUP_WITHOUT_ANNOTATION: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
    @Participants:\tPAR Participant\n@ID:\teng|corpus|PAR|||||Participant|||\n\
    *PAR:\t<I don't> &-uh I know xxx .\n@End\n";

/// Assert `chatter validate` (with `parser_args`, e.g. `["--parser","re2c"]`)
/// rejects inline `content` with `code`, through the real CLI.
fn assert_inline_rejected(
    content: &str,
    code: &str,
    parser_args: &[&str],
) -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir().map_err(|e| TestError::Failure(format!("tempdir: {e}")))?;
    let path = write_fixture(dir.path(), "session.cha", content)?;
    let mut args = vec!["--force"];
    args.extend_from_slice(parser_args);
    let output = harness.run_validate(&path, &args)?;
    let text = combined_output(&output);
    assert!(
        text.contains(code),
        "expected {code} for a `<...>` group recovered via a synthetic MISSING node, got:\n{text}"
    );
    Ok(())
}

/// The default (tree-sitter) parser surfaces the MISSING recovery as E342.
#[test]
fn group_without_annotation_is_rejected_via_cli() -> Result<(), TestError> {
    assert_inline_rejected(GROUP_WITHOUT_ANNOTATION, "E342", &[])
}

/// The re2c oracle must reject the same input (its MISSING-Token Recovery Policy
/// requires emitting a matching diagnostic, not treating it as a known
/// divergence), so the two parsers agree on validity.
#[test]
fn group_without_annotation_is_rejected_by_re2c() -> Result<(), TestError> {
    assert_inline_rejected(GROUP_WITHOUT_ANNOTATION, "E342", &["--parser", "re2c"])
}
