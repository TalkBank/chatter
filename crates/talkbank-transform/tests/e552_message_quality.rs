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

//! E552 diagnostic quality: the message must name WHERE the timing evidence
//! was found and offer the remedy that matches it.
//!
//! Field report (2026-07-07): a transcript with `@Media: ..., unlinked` and an
//! erroneously present `%wor` tier produced "the transcript has timing
//! bullets; remove `unlinked` ... (the media is in fact linked)". The user
//! could see no bullets (they were invisible control characters inside %wor),
//! and the advice was wrong for the actual problem (the stale %wor tier should
//! go, not the `unlinked` qualifier). Real CLAN CHECK 124 does not fire on
//! %wor-only timing (grounded 2026-07-07), so this case is chatter-stricter
//! modernization and its message must carry the explanation on its own.
//!
//! Run with: `cargo nextest run -p talkbank-transform --test e552_message_quality`

use std::fs;
use std::path::PathBuf;

use talkbank_model::{ParseError, ParseValidateOptions};
use talkbank_transform::{PipelineError, parse_and_validate};

/// Repo root: this crate is `<root>/crates/talkbank-transform`, so pop twice.
fn workspace_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    dir.pop();
    dir
}

/// Validation errors for a committed fixture, raw from `parse_and_validate`.
fn errors_for(rel_fixture: &str) -> Vec<ParseError> {
    let fixture = workspace_root().join(rel_fixture);
    let content = fs::read_to_string(&fixture)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", fixture.display()));
    // Alignment must be on: %wor timing surfaces reach the E552 check via
    // `utt.alignments`, which only populate under alignment processing (the
    // CLI default). Note the corollary, verified by this test's first RED
    // run: `--skip-alignment` also skips %wor-based E552 detection.
    let options = ParseValidateOptions::default()
        .with_validation()
        .with_alignment();
    match parse_and_validate(&content, options) {
        Err(PipelineError::Validation(errors)) => errors,
        Err(PipelineError::Parse(parse_errors)) => parse_errors.errors,
        other => panic!("expected errors from {rel_fixture}, got {other:?}"),
    }
}

fn e552_message(rel_fixture: &str) -> String {
    let errors = errors_for(rel_fixture);
    let e552: Vec<_> = errors
        .iter()
        .filter(|e| e.code.as_str() == "E552")
        .collect();
    assert_eq!(
        e552.len(),
        1,
        "expected exactly one E552 from {rel_fixture}, got: {:?}",
        errors.iter().map(|e| e.code.as_str()).collect::<Vec<_>>()
    );
    e552[0].message.clone()
}

/// %wor-only timing: the message must say the evidence lives in the %wor tier
/// and offer BOTH remedies (the stale-%wor one first, since a user who sees no
/// bullets needs to be told where they are).
#[test]
fn wor_only_timing_message_names_the_wor_tier_and_both_remedies() {
    let msg =
        e552_message("tests/error_corpus/validation_errors/E552_unlinked_with_wor_timing.cha");
    assert!(
        msg.contains("%wor"),
        "message must name the %wor tier as the timing evidence, got: {msg}"
    );
    assert!(
        msg.contains("remove `unlinked`") && msg.contains("%wor tier"),
        "message must offer both remedies (remove `unlinked` OR remove the stale %wor tier), got: {msg}"
    );
    assert!(
        !msg.contains("the media is in fact linked"),
        "must not assert the media is definitely linked when only %wor carries timing, got: {msg}"
    );
}

/// Main-tier bullets: the classic CHECK-124 case keeps its direct advice.
#[test]
fn main_bullet_timing_message_keeps_remove_unlinked_advice() {
    let msg = e552_message(
        "crates/talkbank-parser-tests/tests/check_parity/fixtures/CHECK_124_media_unlinked_with_bullet.cha",
    );
    assert!(
        msg.contains("remove `unlinked`"),
        "main-bullet case keeps the remove-unlinked advice, got: {msg}"
    );
}
