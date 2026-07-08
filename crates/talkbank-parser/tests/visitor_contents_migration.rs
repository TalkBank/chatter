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

//! Characterization tests for the within-utterance CONTENTS iteration as it is
//! migrated onto the generated `GrammarTraversal::extract_contents` visitor
//! (Task 3c of the visitor-driven parser migration).
//!
//! These tests pin the OBSERVABLE behavior of `parse_main_tier_contents` at the
//! real parser boundary (`parse_chat_file_streaming` -> `ChatFile` + collected
//! diagnostics). Task 3c replaces the hand-walked `for idx in 0..child_count`
//! loop + `match child.kind()` string dispatch with `extract_contents` +
//! exhaustive `NodeSlot<ContentsItemChoice>` dispatch, but it is
//! behavior-preserving: the `Vec<UtteranceContent>` model and the recovery
//! diagnostics (especially the `analyze_word_error` path for a parser `ERROR`
//! fragment that lands directly under `contents`) must not change for any
//! reachable input. The migration stops at the `content_item` level: each
//! concrete choice is still handed to the existing `parse_content_item` (whose
//! internals are Task 5).
//!
//! Every expectation below was captured by RUNNING the pre-migration parser
//! (HEAD `033463c`), not guessed:
//!
//! - A valid CA overlap utterance (`corpus/reference/ca/overlaps.cha`) parses to
//!   the expected interleaving of `OverlapPoint` and `Word` content items with
//!   zero diagnostics. This exercises the `Present(ContentItem)` and
//!   `Present(OverlapPoint)` arms in document order.
//! - A valid CA intonation utterance (`corpus/reference/ca/intonation.cha`)
//!   carries a trailing standalone `Separator` content item after its words.
//!   This exercises the `Present(Separator)` arm.
//! - A malformed utterance with a stray `[` fragment in the middle of the words
//!   (`one [x foo two .`) produces a parser `ERROR` node directly under
//!   `contents`; the contents walk routes it through `analyze_word_error`,
//!   emitting exactly one `E375` "Could not parse bracket annotation" at the
//!   fragment span. This exercises the `Error(node)` arm.

use talkbank_model::model::{Utterance, UtteranceContent};

mod common;
use common::parse_utterances_and_diags;

/// A `*CHI:` utterance whose `[x` repetition-count annotation is malformed (a
/// space sits between the word and the bracket, and the count is non-numeric).
/// Tree-sitter recovers by emitting an `ERROR` node holding the whitespace plus
/// the `left_bracket` as a DIRECT child of the `contents` node, so it reaches
/// `parse_main_tier_contents`'s error branch (`analyze_word_error`). Inline
/// input (not a new `.cha`), matching the established migration-test shape.
const MALFORMED_BRACKET: &str = "@UTF8\n@Begin\n*CHI:\tone [x foo two .\n@End\n";

/// Tag one content item by its `UtteranceContent` variant for sequence
/// assertions. `Word` carries its raw text so the ordering is meaningful; the
/// labeled fallback surfaces (rather than silently drops) any variant the
/// fixtures are not expected to produce.
fn content_tag(item: &UtteranceContent) -> String {
    match item {
        UtteranceContent::Word(word) => format!("Word({})", word.raw_text()),
        UtteranceContent::OverlapPoint(_) => "OverlapPoint".to_string(),
        UtteranceContent::Separator(_) => "Separator".to_string(),
        other => format!("other:{other:?}"),
    }
}

/// The tagged content sequence of an utterance's main tier.
fn content_tags(utt: &Utterance) -> Vec<String> {
    utt.main.content.content.iter().map(content_tag).collect()
}

#[test]
fn valid_overlap_contents_parse_to_interleaved_overlap_and_word_items() {
    let input = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/ca/overlaps.cha"
    ))
    .expect("read corpus/reference/ca/overlaps.cha");

    let (utterances, diags) = parse_utterances_and_diags(&input);

    assert!(
        diags.is_empty(),
        "valid overlaps file must produce zero diagnostics, got: {diags:?}"
    );

    // First utterance is `*SPK:\t⌈ one ⌉ ⌈2 two ⌉2 .`: the contents are the
    // interleaved overlap points and words (the trailing `.` is the terminator,
    // which lives in `utterance_end`, not in `contents`).
    let first = &utterances[0];
    assert_eq!(first.main.speaker.as_str(), "SPK");
    assert_eq!(
        content_tags(first),
        vec![
            "OverlapPoint",
            "Word(one)",
            "OverlapPoint",
            "OverlapPoint",
            "Word(two)",
            "OverlapPoint",
        ],
        "overlap utterance must interleave OverlapPoint and Word content items"
    );
}

#[test]
fn valid_intonation_contents_carry_trailing_separator_item() {
    let input = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/ca/intonation.cha"
    ))
    .expect("read corpus/reference/ca/intonation.cha");

    let (utterances, diags) = parse_utterances_and_diags(&input);

    assert!(
        diags.is_empty(),
        "valid intonation file must produce zero diagnostics, got: {diags:?}"
    );

    // First utterance is `*SPK:\trising to high ⇗`: three words followed by a
    // standalone `rising_to_high` CA marker, which the grammar wraps in a
    // `separator` node -> a `Separator` content item.
    let first = &utterances[0];
    assert_eq!(first.main.speaker.as_str(), "SPK");
    assert_eq!(
        content_tags(first),
        vec!["Word(rising)", "Word(to)", "Word(high)", "Separator"],
        "intonation utterance must end with a standalone Separator content item"
    );
}

#[test]
fn malformed_bracket_fragment_emits_exact_word_error_diagnostic() {
    let (utterances, diags) = parse_utterances_and_diags(MALFORMED_BRACKET);

    // EXACTLY one diagnostic, captured from the pre-migration parser: the stray
    // `[` fragment under `contents` is classified by `analyze_word_error` as
    // E375 "Could not parse bracket annotation" at the fragment span.
    assert_eq!(
        diags,
        vec![(
            "E375".to_string(),
            22,
            24,
            "Could not parse bracket annotation".to_string(),
        )],
        "malformed bracket fragment must emit exactly one E375 at span (22..24)"
    );

    // The surrounding words still parse into the model: the ERROR fragment does
    // not abort the contents walk.
    assert_eq!(utterances.len(), 1, "exactly one utterance");
    let first = &utterances[0];
    assert_eq!(first.main.speaker.as_str(), "CHI");
    assert_eq!(
        content_tags(first),
        vec!["Word(one)", "Word(x)", "Word(foo)", "Word(two)"],
        "the four words around the stray bracket fragment must all parse"
    );
}
