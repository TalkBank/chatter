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

//! Regression test for the E207 bare-`&` suggestion text.
//!
//! Background: the canonical CHAT changelog (`changes.txt`) retired the
//! bare `&XYZ` form long ago. Modern CHAT uses four marker prefixes:
//!
//! - `&-XYZ` filler  (`changes.txt` line 537)
//! - `&+XYZ` fragment (changes.txt line 539, reassigned from "incomplete")
//! - `&~XYZ` nonword  (changes.txt line 175, replaced the bare-`&` "fragment")
//! - `&=XYZ` event    (longstanding)
//!
//! The E207 suggestion in `errors.rs` previously said `'&uh' for filler`,
//! which itself fails to parse, because bare `&uh` is the very pattern E207
//! rejects. The suggestion contradicted the parser's own behavior.

mod common;

use common::parse_and_collect_errors;

/// Minimal CHAT fragment containing a bare `&um` that E207 rejects.
const CHAT_WITH_BARE_AMP: &str = "@UTF8\n\
    @Begin\n\
    @Languages:\teng\n\
    @Participants:\tCHI Target_Child\n\
    @ID:\teng|corpus|CHI|||||Target_Child|||\n\
    *CHI:\t&um something .\n\
    @End\n";

/// Discriminator: `ErrorCode::UnknownAnnotation` is also raised by the
/// `[@ xyz]` scoped-annotation path, so we additionally key on the
/// bare-`&` message substring to pick the right E207 variant.
fn e207_bare_amp_suggestion(input: &str) -> String {
    parse_and_collect_errors(input)
        .into_iter()
        .find(|e| e.message.contains("must be followed by annotation name"))
        .expect("expected an E207 bare-`&` error")
        .suggestion
        .expect("E207 must carry a help suggestion")
}

#[test]
fn e207_suggestion_uses_canonical_prefixed_filler_form() {
    let suggestion = e207_bare_amp_suggestion(CHAT_WITH_BARE_AMP);
    assert!(
        suggestion.contains("&-uh") || suggestion.contains("&-um"),
        "suggestion should reference the canonical prefixed filler form, got: {suggestion:?}"
    );
}

#[test]
fn e207_suggestion_does_not_reference_retired_bare_amp_form() {
    let suggestion = e207_bare_amp_suggestion(CHAT_WITH_BARE_AMP);
    // The pre-fix suggestion was `"Complete the annotation like '&=laugh'
    // or '&uh' for filler"`. The literal `'&uh'` is retired; modern CHAT
    // uses `'&-uh'`. Pin against any quoted-bare-`&uh`-style recurrence.
    for forbidden in ["'&uh'", "'&um'", "'&mhm'"] {
        assert!(
            !suggestion.contains(forbidden),
            "suggestion must not reference the retired bare-`&` form {forbidden}, got: {suggestion:?}"
        );
    }
}

#[test]
fn e207_suggestion_covers_all_four_canonical_marker_kinds() {
    let suggestion = e207_bare_amp_suggestion(CHAT_WITH_BARE_AMP);
    // All four canonical prefixes per java-chatter changes.txt should
    // appear, so a confused contributor reading a single error message
    // learns the current four-way distinction (filler / fragment /
    // nonword / event).
    for prefix in ["&-", "&+", "&~", "&="] {
        assert!(
            suggestion.contains(prefix),
            "suggestion should reference the {prefix} marker kind, got: {suggestion:?}"
        );
    }
}
