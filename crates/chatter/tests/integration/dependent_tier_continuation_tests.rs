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

//! Dependent-tier CONTINUATION lines may carry leading whitespace.
//!
//! A CHAT tier continues onto the next line when that line begins with a tab.
//! Transcribers routinely leave an incidental space after that tab, so the
//! continuation reads `\t <content>` rather than `\t<content>`. That space is
//! meaningless: on a free-text tier such as `%com` it is not even content.
//!
//! chatter rejected it. `\t00:45:00` validated, `\t 00:45:00` produced E316
//! ("Unparsable content on dependent tier") plus E330, and the resulting parse
//! taint suppressed the main/%mor and %mor/%gra alignment checks (E600), so a
//! single stray space silently disabled real validation on the file.
//!
//! ADJUDICATION (2026-07-29). CLAN CHECK accepts these files: run over the
//! affected transcripts with and without the space, its verdict is
//! byte-identical. CHECK's silence is not by itself proof chatter is wrong,
//! since chatter is deliberately stricter in many places, so the governing
//! test was applied instead: does the construct FAIL TO MAKE SENSE? A space
//! before comment text does not. Two further tells confirm a defect rather
//! than a rule: E330's message claimed "bullet content" on a line containing
//! no bullet, and E316 is a generic unparsable-content error with no spec
//! saying leading whitespace is illegal.
//!
//! Fixed in the token, in both parsers: `grammar.js`'s `continuation` and its
//! re2c mirror in `lexer.re`. Spaces only, never a second tab, which is
//! structural. Scope is narrow on purpose: a space BEFORE the tab is not a
//! continuation at all, and only the three rules that reference
//! `$.continuation` are affected, all of them free-text or text-with-bullets
//! tiers where no interior spacing is load-bearing.
//!
//! This was found the expensive way. Eight real corpus files across two data
//! repositories were edited to strip the space before anyone asked whether
//! chatter was at fault; those edits have been reverted.


use crate::common::{CliHarness, stdout_string, write_fixture};
use talkbank_parser_tests::test_error::TestError;

/// Header block shared by every fixture below.
const HEADER: &str = "@UTF8\n@Begin\n@Languages:\tdeu\n@Participants:\tCHI Target_Child\n@ID:\tdeu|corpus|CHI|||||Target_Child|||\n";

/// Assert the fixture validates cleanly through the real CLI.
///
/// Runs under `CliHarness` so the validation cache is isolated per test; a
/// bare `cargo_bin_cmd!` would read and write the developer's real cache.
fn assert_valid(name: &str, body: &str) -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let path = write_fixture(harness.home_dir(), name, body)?;
    let output = harness.run_validate(&path, &[])?;
    let stdout = stdout_string(&output);
    assert!(
        output.status.success() && stdout.contains("Invalid: 0"),
        "expected {name} to validate cleanly, got:\n{stdout}"
    );
    Ok(())
}

/// The control: a continuation line with NO leading space already validated,
/// and must keep doing so. If this ever fails the fix went too far.
#[test]
fn continuation_without_leading_space_is_valid() -> Result<(), TestError> {
    let body = format!("{HEADER}*CHI:\thallo .\n%com:\tein kommentar .\n\tnoch text .\n@End\n");
    assert_valid("plain.cha", &body)
}

/// The bug, in the exact shape found in the wild: a continued `%com` whose
/// second line carries a stray space before a timestamp, followed by a third
/// line without one. Covers both the minimal case and the real one.
#[test]
fn continuation_with_leading_space_is_valid() -> Result<(), TestError> {
    let body = format!(
        "{HEADER}*CHI:\thallo .\n%com:\tMot und Cla reden .\n\t 00:45:00\n\tJon hat die Arzt Utensilien .\n@End\n"
    );
    assert_valid("wild_shape.cha", &body)
}

/// A run of several spaces after the tab is the same incidental whitespace.
#[test]
fn continuation_with_multiple_leading_spaces_is_valid() -> Result<(), TestError> {
    let body = format!("{HEADER}*CHI:\thallo .\n%com:\tein kommentar .\n\t    noch text .\n@End\n");
    assert_valid("multi_space.cha", &body)
}

/// The stray space must not suppress alignment validation. Before the fix the
/// parse taint from E316 caused E600 to skip main/%mor and %mor/%gra checks,
/// which is the damaging part: a whitespace typo silently disabled real
/// validation. Here `%mor` has FEWER items than the main tier has words, so a
/// genuine alignment error must still surface.
#[test]
fn leading_space_does_not_suppress_alignment_checking() -> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let body = format!(
        "{HEADER}*CHI:\thallo welt .\n%mor:\tco|hallo .\n%com:\tein kommentar .\n\t 00:45:00\n@End\n"
    );
    let path = write_fixture(harness.home_dir(), "alignment.cha", &body)?;
    let stdout = stdout_string(&harness.run_validate(&path, &["--format", "json"])?);
    assert!(
        !stdout.contains("E600"),
        "alignment checking was skipped because of the leading space; E600 still present:\n{stdout}"
    );
    Ok(())
}
