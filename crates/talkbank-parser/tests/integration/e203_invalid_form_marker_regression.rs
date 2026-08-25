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

//! Regression test for E203 invalid-form-marker, re-homed off ERROR-text scanning.
//!
//! Bug history: an unknown form marker (e.g. `word@zz`) was historically also
//! classified by scanning the raw text of an ERROR node, via
//! `find_invalid_form_marker_offset` in `analyze_word_error` (and a sibling `@`
//! text-scan in `analyze_utterance_error`). That ERROR-text classification is the
//! banned anti-pattern.
//!
//! Re-home: `word@zz` PARSES into a structured word with a `form_marker` child
//! (`@zz`). The parser's typed dispatch reads that parsed `form_marker` node's
//! own text and hands it to `FormType::from_payload`, which owns the question
//! of which markers exist; `zz` is not one of them, so E203 (`InvalidFormType`)
//! is emitted. Reading a parsed node's own content for validation is
//! typed-model work, NOT raw-CHAT / ERROR-text scanning.
//!
//! This comment used to enumerate the valid set, and by 2026-08-11 it was the
//! last place in the repository still advertising `@a`, one commit after `@a`
//! was retired. The set has one owner now,
//! `spec/form_markers/form_marker_registry.json`; a copy here could only ever
//! go stale again, because nothing reads a comment.
//! The redundant ERROR-text branches are removed; this test pins that the typed
//! path still flags `@zz`, does NOT regress to generic E316, and does NOT
//! false-positive on valid markers (`@i`, `@s:eng`).

use talkbank_model::ErrorCollector;
use talkbank_model::model::FileStem;
use talkbank_model::model::TranscriptName;
use talkbank_model::model::WriteChat;
use talkbank_parser::TreeSitterParser;

/// An unknown form marker `word@zz` must be flagged E203 via the typed
/// `form_marker` dispatch, and must NOT regress to a generic E316.
#[test]
fn unknown_form_marker_emits_e203_not_e316() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\tword@zz .\n@End\n";

    let diags = crate::common::parse_validate_and_collect_diagnostics(
        input,
        TranscriptName::Named(FileStem::from_stem("e203_regression")),
    );
    let codes: Vec<&str> = diags.iter().map(|(c, _)| c.as_str()).collect();

    assert!(
        codes.contains(&"E203"),
        "Expected E203 (invalid form type) for `word@zz`, got: {diags:#?}",
    );
    assert!(
        !codes.contains(&"E316"),
        "`word@zz` must not regress to generic E316 (unparsable content); got: {diags:#?}",
    );
}

/// A valid built-in form marker `hello@i` (interjection) must NOT be flagged
/// E203 (no false positive) and must not produce E316.
#[test]
fn valid_builtin_form_marker_not_flagged() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello@i .\n@End\n";

    let diags = crate::common::parse_validate_and_collect_diagnostics(
        input,
        TranscriptName::Named(FileStem::from_stem("e203_regression")),
    );
    let codes: Vec<&str> = diags.iter().map(|(c, _)| c.as_str()).collect();

    assert!(
        !codes.contains(&"E203"),
        "Valid marker `hello@i` must NOT be flagged E203; got: {diags:#?}",
    );
    assert!(
        !codes.contains(&"E316"),
        "Valid marker `hello@i` must not produce E316; got: {diags:#?}",
    );
}

/// A valid language suffix `hello@s:eng` must NOT be flagged E203 (its base `s`
/// is a language tag, not a form marker) and must not produce E316.
#[test]
fn valid_language_suffix_not_flagged() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello@s:eng .\n@End\n";

    let diags = crate::common::parse_validate_and_collect_diagnostics(
        input,
        TranscriptName::Named(FileStem::from_stem("e203_regression")),
    );
    let codes: Vec<&str> = diags.iter().map(|(c, _)| c.as_str()).collect();

    assert!(
        !codes.contains(&"E203"),
        "Valid language suffix `hello@s:eng` must NOT be flagged E203; got: {diags:#?}",
    );
    assert!(
        !codes.contains(&"E316"),
        "Valid language suffix `hello@s:eng` must not produce E316; got: {diags:#?}",
    );
}

/// An undeclared marker survives a parse-and-serialize round trip byte for
/// byte.
///
/// # Why this exists
///
/// The recovery path used to store `FormType::UserDefined(payload)` for
/// `word@zz`, which asserts that the word carries the `@z` user-defined marker
/// with label `zz`. `Word::write_chat` rebuilds the marker from `form_type`
/// rather than from the raw text, so it wrote `word@z:zz`: a silent
/// corruption, unobservable only because every command that serializes aborts
/// on E203 first. That is a latent trap, not a safe state, and the re2c parser
/// stored nothing at all for the same input, so the two parsers disagreed.
///
/// A ROUNDTRIP test rather than an assertion about which variant is stored,
/// deliberately: this pins the property that matters (the transcript comes
/// back unchanged), which no type signature can state, and it still holds if
/// the recovery representation changes again.
#[test]
fn an_undeclared_form_marker_round_trips_unchanged() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
                 @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\tword@zz .\n@End\n";

    let parser = TreeSitterParser::new().expect("parser");
    let errors = ErrorCollector::new();
    let chat_file = parser.parse_chat_file_streaming(input, &errors);

    let mut serialized = String::new();
    chat_file.write_chat(&mut serialized).expect("serialize");

    assert_eq!(
        serialized, input,
        "an undeclared form marker must serialize back verbatim; storing it as \
         a DECLARED marker rewrote `word@zz` as `word@z:zz`"
    );
}
