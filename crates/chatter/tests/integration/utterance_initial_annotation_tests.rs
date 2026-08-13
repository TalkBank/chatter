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

//! An annotation that opens an utterance must be diagnosed ACCURATELY.
//!
//! `*CHI:\t[: closed] .` is a correctly closed replacement with nothing before
//! it. Its one fault is that a replacement scopes over preceding material and
//! there is none, which is E759 (CLAN CHECK rule 52). Real CLAN CHECK reports
//! exactly that and nothing else.
//!
//! chatter also reported E305 "Missing terminator in main tier" on a line whose
//! terminator is right there and IS in the parse tree
//! (`ending: (utterance_end (period) (newline))`). The lowering dropped it when
//! an ERROR node preceded `tier_body`, so the model's rule, which is correct,
//! was handed a `None` it should never have seen.
//!
//! Why this is worth a CLI test rather than a unit test: the whole point is
//! what a USER is told. The project's own cautionary tale is the predecessor
//! Java tool, of which the corpus authority said he never read the error
//! messages because they were wrong or useless. A validator earns its
//! diagnostics one accurate message at a time.
//!
//! Full adjudication, including the re2c side:
//! `docs/audits/2026-08-11-utterance-initial-annotation-adjudication.md`.

use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

use crate::common::{CliHarness, combined_output, write_fixture};

/// A closed replacement at utterance start: E759, and nothing about a
/// terminator.
#[test]
fn closed_replacement_at_utterance_start_does_not_claim_a_missing_terminator()
-> Result<(), TestError> {
    let harness = CliHarness::new()?;
    let dir = tempdir().map_err(|e| TestError::Failure(format!("tempdir: {e}")))?;
    let path = write_fixture(
        dir.path(),
        "session.cha",
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
         @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\t[: closed] .\n@End\n",
    )?;

    let output = harness.run_validate(&path, &["--force"])?;
    let text = combined_output(&output);

    assert!(
        text.contains("E759"),
        "the real fault, a replacement with nothing to scope over, must be \
         reported (E759, CLAN CHECK 52). Got:\n{text}"
    );
    assert!(
        !text.contains("E305"),
        "the utterance ENDS with ` .` and the terminator is in the parse tree, \
         so claiming it is missing is a wrong message, which is how a validator \
         teaches users to stop reading its output. Got:\n{text}"
    );

    // The cascade that followed from the same loss. Recovery leaves this
    // utterance holding one zero-width separator, so the "empty content"
    // checks fired on a line that visibly has content. The parse-taint
    // exemption already existed for a genuinely EMPTY content list and was
    // missing on its only-separators sibling.
    for cascaded in ["E306", "E253"] {
        assert!(
            !text.contains(cascaded),
            "{cascaded} says this utterance has no content, on a line whose \
             content is right there in the source. It is a consequence of \
             recovery, not a fact about the file. Got:\n{text}"
        );
    }
    Ok(())
}
