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

//! Where a retrace marker sits among its sibling annotations, end to end
//! through the CLI.
//!
//! A content item followed by a run of scoped markers is a LEFT-ASSOCIATIVE
//! CHAIN: each marker scopes over everything to its left. So
//!
//! ```text
//! dog [* p:w] [/] dog     the error is on the abandoned attempt
//! dog [/] [* p:w] dog     the error is on the retrace
//! ```
//!
//! are different claims, and a writer that turns the first into the second has
//! changed what the transcriber said.
//!
//! **Why these are CLI tests and not unit tests.** The defect is invisible at
//! every level below the byte stream: `chatter validate` calls both lines
//! valid, `validate --roundtrip` reports zero failures because roundtrip tests
//! idempotence of `serialize(parse(·))` rather than fidelity to the input, and
//! `SemanticEq` holds because the two orderings produce the IDENTICAL model.
//! Only comparing written bytes against source bytes can see it, so that is
//! what these tests do.
//!
//! Grounding: 12,226 places in the corpora write an annotation immediately
//! before a retrace marker, dominated by `[* code]` error codes. Design note:
//! `docs/design/2026-08-07-retrace-model-and-the-lost-marker-position.md`.

use crate::common::Verdict;
use std::fs;
use talkbank_model::ErrorCode;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

/// Build a one-utterance English file whose main tier is exactly `words`.
fn chat_file(words: &str) -> String {
    format!(
        "@UTF8\n\
         @Begin\n\
         @Languages:\teng\n\
         @Participants:\tCHI Target_Child\n\
         @ID:\teng|corpus|CHI|3;00.00|male|||Target_Child|||\n\
         *CHI:\t{words}\n\
         @End\n",
    )
}

/// Assert that `chatter normalize` writes the main tier back unchanged.
///
/// Byte comparison of the ONE line under test rather than of the whole file,
/// because normalize legitimately touches headers and a whole-file diff would
/// fail for reasons that have nothing to do with marker order.
fn assert_main_tier_preserved(name: &str, words: &str) -> Result<(), TestError> {
    let dir = tempdir()?;
    let path = dir.path().join(name);
    fs::write(&path, chat_file(words))?;

    let output = crate::common::chatter_cmd()
        .arg("normalize")
        .arg(&path)
        .output()?;
    let written = String::from_utf8_lossy(&output.stdout);
    let main_tier = written
        .lines()
        .find(|line| line.starts_with("*CHI:"))
        .map(|line| line.trim_start_matches("*CHI:\t").to_owned());

    assert_eq!(
        main_tier.as_deref(),
        Some(words),
        "normalize rewrote the main tier of {name}"
    );
    Ok(())
}

/// Assert `chatter validate`'s verdict on a one-utterance file of `words`.
///
/// Thin wrapper over the shared `common::assert_validation`, which owns the
/// two-stream assertion and the `Verdict` type; this only supplies the file.
fn assert_validation(name: &str, words: &str, expected: Verdict) -> Result<(), TestError> {
    crate::common::assert_validation(name, &chat_file(words), expected)
}

// ---------------------------------------------------------------------------
// Order across the marker must survive a write
// ---------------------------------------------------------------------------

/// The dominant attested shape: an error code on the retraced material.
#[test]
fn an_error_code_before_the_marker_stays_before_it() -> Result<(), TestError> {
    assert_main_tier_preserved("error_before.cha", "dog [* p:w] [/] dog .")
}

/// The mirror image, which happens to be the form the writer already emits.
/// Present so a fix that simply swapped the two orderings could not pass.
#[test]
fn an_error_code_after_the_marker_stays_after_it() -> Result<(), TestError> {
    assert_main_tier_preserved("error_after.cha", "dog [/] [* p:w] dog .")
}

/// The one occurrence anywhere in the IISRP workstream, kept as a fixture so
/// the corpus we actually ship has a named regression guard.
#[test]
fn a_percent_comment_before_the_marker_stays_before_it() -> Result<(), TestError> {
    assert_main_tier_preserved("percent_before.cha", "then [% pause] [//] and .")
}

/// An overlap marker is a claim about WHICH stretch overlapped, so moving it
/// across the retrace marker changes a timing assertion rather than a label.
#[test]
fn an_overlap_marker_before_the_marker_stays_before_it() -> Result<(), TestError> {
    assert_main_tier_preserved("overlap_before.cha", "then [<] [//] and .")
}

/// Group form, to pin that the rule is about the marker sequence and not about
/// whether the retraced material was bracketed.
#[test]
fn an_error_code_before_a_group_marker_stays_before_it() -> Result<(), TestError> {
    assert_main_tier_preserved("group_before.cha", "<the dog> [* s:r] [/] the dog .")
}

// ---------------------------------------------------------------------------
// The two shapes that are errors
// ---------------------------------------------------------------------------

/// Brian MacWhinney, 2026-08-07, asked about `на [//] [/] на`: "clearly a
/// mistake ... It's an error." 105 occurrences in 46 corpus files.
///
/// Today chatter accepts this AND silently writes back `a [/] a .`, demoting a
/// retracing-with-correction to a repetition, because the second marker
/// overwrites the first in a field that holds only one.
#[test]
fn validate_rejects_two_adjacent_retrace_markers() -> Result<(), TestError> {
    assert_validation(
        "adjacent.cha",
        "a [//] [/] a .",
        Verdict::Rejected(ErrorCode::RetraceWithNoMaterial),
    )
}

/// The bracketed spelling of the same error: a retrace whose content is
/// another retrace ALONE, with no word of its own. Four occurrences corpus-wide
/// (typed-AST scan, 2026-08-07).
#[test]
fn validate_rejects_a_retrace_wrapping_only_a_retrace() -> Result<(), TestError> {
    assert_validation(
        "wordless_wrapper.cha",
        "<<a> [/]> [//] b .",
        Verdict::Rejected(ErrorCode::RetraceWithNoMaterial),
    )
}

/// Brian MacWhinney, 2026-08-07, asked whether a retracing marker on a bare
/// event means anything: **"No, not legal. You can't retrace a laugh."**
///
/// A marker retraces the WORDS to its left, and a laugh is not a word. This is
/// the unbracketed spelling, taken from `dementia-data/English/WLS/02/02393.cha`.
#[test]
fn validate_rejects_a_retrace_over_a_bare_event() -> Result<(), TestError> {
    assert_validation(
        "bare_event.cha",
        "&=laughs [//] water .",
        Verdict::Rejected(ErrorCode::RetraceWithoutWords),
    )
}

/// The bracketed spelling, which is the one that actually dominates: a
/// vocalization repeated, with the group holding nothing but the event.
/// Attested as `<&=sigh> [/] &=sigh`, `<&=eh> [/] <&=eh> [/] &=eh`, and
/// `<&=ih> [/] &=ih`.
#[test]
fn validate_rejects_a_retrace_over_a_grouped_event() -> Result<(), TestError> {
    assert_validation(
        "grouped_event.cha",
        "<&=sigh> [/] &=sigh ok .",
        Verdict::Rejected(ErrorCode::RetraceWithoutWords),
    )
}

/// A pause is not a word either, so padding the group with one does not rescue
/// it. Attested verbatim as `<(.) &=laughs> [//]` in `slabank-data`.
#[test]
fn validate_rejects_a_retrace_over_a_pause_and_an_event() -> Result<(), TestError> {
    assert_validation(
        "pause_and_event.cha",
        "<(.) &=laughs> [//] ok .",
        Verdict::Rejected(ErrorCode::RetraceWithoutWords),
    )
}

// ---------------------------------------------------------------------------
// What must NOT be caught, and this is the point of the narrow phrasing
// ---------------------------------------------------------------------------

/// The form the CHAT maintainer proposed INSTEAD, half an hour before ruling
/// the bare event illegal: put the laugh inside material that has words, and
/// retrace that. So the rule cannot be "the retraced material contains an
/// event"; it has to be "the retraced material contains no word".
#[test]
fn validate_accepts_a_retrace_over_a_group_of_words_containing_an_event() -> Result<(), TestError> {
    assert_validation(
        "event_among_words.cha",
        "<the floor on the &=laughs water> [//] the floor .",
        Verdict::Valid,
    )
}

/// 205 corpus sites wrap an annotated group or a quotation, which hold their
/// words one level down. A rule testing for a DIRECT word child would reject
/// every one of them.
#[test]
fn validate_accepts_a_retrace_whose_words_are_one_level_down() -> Result<(), TestError> {
    assert_validation(
        "words_one_level_down.cha",
        "<<the dog> [?]> [/] the dog .",
        Verdict::Valid,
    )
}

/// A retrace whose scope covers words that were themselves repeated. Ordinary
/// disfluency, 11,159 occurrences, concentrated in the aphasia and fluency
/// corpora whose subject this is. A rule reading "no `Retrace` inside a
/// `Retrace`" would invalidate every one of them.
#[test]
fn validate_accepts_a_retrace_over_words_that_were_repeated() -> Result<(), TestError> {
    assert_validation(
        "nested_with_words.cha",
        "<the [/] the piece> [//] the people .",
        Verdict::Valid,
    )
}

/// The long stutter chains that dominate aphasia transcription.
#[test]
fn validate_accepts_a_long_stutter_chain() -> Result<(), TestError> {
    assert_validation(
        "stutter_chain.cha",
        "<we [/] we [/] we [/] we did> [//] when I needed to .",
        Verdict::Valid,
    )
}

/// Annotations after the marker are legal and common: 8,766 occurrences across
/// ten annotation kinds.
#[test]
fn validate_accepts_an_annotation_after_the_marker() -> Result<(), TestError> {
    assert_validation(
        "trailing_annotation.cha",
        "<a b> [/] [= gloss] a b .",
        Verdict::Valid,
    )
}

/// A retrace over an annotated group, the bracketed form of the
/// annotation-before-marker chain. 128 occurrences as `[/] { annotated-group }`.
#[test]
fn validate_accepts_a_retrace_over_an_annotated_group() -> Result<(), TestError> {
    assert_validation(
        "annotated_group_inside.cha",
        "<<a b> [?]> [/] a b .",
        Verdict::Valid,
    )
}

/// The rule must reach retraces nested inside phonological and sign groups.
///
/// `marker_on_marker.rs` listed `PhoGroup`/`SinGroup` as leaves ("nothing here
/// can contain a retrace"), while its sibling `detection.rs` recurses into
/// both and `convert_to_group_content` maps both into bracketed content. So a
/// marker-on-marker inside `‹...›` escaped the rule entirely.
///
/// Found by a review agent reading the two leaf-sets against each other, not by
/// any test: the same disagreement-between-twin-walkers that let
/// `AnnotatedRetrace` bypass `detection.rs` when the variant was added.
#[test]
fn validate_rejects_a_marker_on_a_marker_inside_a_pho_group() -> Result<(), TestError> {
    assert_validation(
        "pho_group_nested.cha",
        "‹a [//] [/] a› b .",
        Verdict::Rejected(ErrorCode::RetraceWithNoMaterial),
    )
}
